use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

mod mapping;

use super::repo_id;
use crate::contexts::daemon::domain::cache::{CacheEntry, RepoId};
use crate::contexts::daemon::domain::refresh_policy::RefreshPolicy;
use crate::shared::protocol::{EntryDto, PrOutput, Request, Response};
use crate::shared::refresh_mode::RefreshMode;
use mapping::entry_to_dtos;

pub(super) struct ActionResult {
    pub(super) response: Response,
    /// `Some(repo_id)` のとき、ロック解放後にリフレッシュを起動する
    pub(super) refresh_repo_id: Option<RepoId>,
    pub(super) refresh_cwd: Option<PathBuf>,
    pub(super) stop: bool,
}

#[derive(Debug, Clone, Copy)]
enum RefreshState {
    NeedsRefresh,
    Refreshing,
}

struct StaleQueryParams {
    output: String,
    has_fetched: bool,
    stored_cwd: PathBuf,
    refresh_state: RefreshState,
}

pub(super) fn process(
    request: &Request,
    entries: &mut HashMap<RepoId, CacheEntry>,
    policy: &RefreshPolicy,
    started_at: Instant,
    ttl: u64,
) -> ActionResult {
    match request {
        Request::Query { cwd } => {
            let Some((repo_id_str, branch)) = repo_id::repo_info_from_cwd(cwd) else {
                return ActionResult {
                    response: Response::Output {
                        output: String::new(),
                    },
                    refresh_repo_id: None,
                    refresh_cwd: None,
                    stop: false,
                };
            };

            let repo_id = RepoId::new(repo_id_str);
            let cwd_path = PathBuf::from(cwd);
            process_query(&repo_id, branch, cwd_path, ttl, entries, policy)
        }
        Request::Update {
            repo_id,
            output,
            refresh_mode,
            pr_outputs,
        } => process_update(
            &RepoId::new(repo_id.clone()),
            output,
            *refresh_mode,
            pr_outputs,
            entries,
        ),
        Request::Stop => ActionResult {
            response: Response::Ok,
            refresh_repo_id: None,
            refresh_cwd: None,
            stop: true,
        },
        Request::Status => {
            let uptime_secs = started_at.elapsed().as_secs();
            let entry_count = entries.len();
            ActionResult {
                response: Response::Status {
                    entries: entry_count,
                    uptime_secs,
                    version: env!("CARGO_PKG_VERSION").to_owned(),
                },
                refresh_repo_id: None,
                refresh_cwd: None,
                stop: false,
            }
        }
        Request::Entries => {
            let dtos: Vec<EntryDto> = entries.values().flat_map(entry_to_dtos).collect();
            ActionResult {
                response: Response::Entries { entries: dtos },
                refresh_repo_id: None,
                refresh_cwd: None,
                stop: false,
            }
        }
    }
}

fn process_query(
    repo_id: &RepoId,
    branch: String,
    cwd_path: PathBuf,
    ttl: u64,
    entries: &mut HashMap<RepoId, CacheEntry>,
    policy: &RefreshPolicy,
) -> ActionResult {
    match entries.get_mut(repo_id) {
        Some(entry) if entry.is_fresh(policy.effective_ttl(entry, ttl)) => {
            // Fresh キャッシュ
            entry.record_query();
            ActionResult {
                response: Response::Output {
                    output: entry.output().to_owned(),
                },
                refresh_repo_id: None,
                refresh_cwd: None,
                stop: false,
            }
        }
        Some(entry) => {
            // Stale または TTL 超過
            let output = entry.output().to_owned();
            let has_fetched = entry.has_fetched();
            let stored_cwd = entry.cwd().to_path_buf();
            let refresh_state = if entry.is_refreshing() {
                RefreshState::Refreshing
            } else {
                RefreshState::NeedsRefresh
            };
            let is_terminal = entry.refresh_mode() == RefreshMode::Terminal;
            // Query を受けたので last_queried_at を更新し Cold カウンタをリセット
            let was_cold = entry.is_cold_or_never_queried(policy.warm_to_cold_secs);
            if was_cold {
                entry.reset_cold_count();
            }
            entry.record_query();
            if is_terminal && matches!(refresh_state, RefreshState::NeedsRefresh) {
                // Terminal が stale になったらモードをリセットして再確認
                entry.reset_to_warm();
            }
            process_stale_query(
                repo_id,
                StaleQueryParams {
                    output,
                    has_fetched,
                    stored_cwd,
                    refresh_state,
                },
                entries,
            )
        }
        None => {
            // 初回 Miss → エントリを作成してリフレッシュ予約
            entries.insert(
                repo_id.to_owned(),
                CacheEntry::new(cwd_path.clone(), branch, ttl),
            );
            ActionResult {
                response: Response::Output {
                    output: "? loading".to_owned(),
                },
                refresh_repo_id: Some(repo_id.to_owned()),
                refresh_cwd: Some(cwd_path),
                stop: false,
            }
        }
    }
}

fn process_stale_query(
    repo_id: &RepoId,
    params: StaleQueryParams,
    entries: &mut HashMap<RepoId, CacheEntry>,
) -> ActionResult {
    let StaleQueryParams {
        output,
        has_fetched,
        stored_cwd,
        refresh_state,
    } = params;

    match (refresh_state, has_fetched) {
        (RefreshState::Refreshing, false) => ActionResult {
            response: Response::Output {
                output: "? loading".to_owned(),
            },
            refresh_repo_id: None,
            refresh_cwd: None,
            stop: false,
        },
        (RefreshState::Refreshing, true) => ActionResult {
            response: Response::Output { output },
            refresh_repo_id: None,
            refresh_cwd: None,
            stop: false,
        },
        (RefreshState::NeedsRefresh, _) => {
            entries
                .get_mut(repo_id)
                .expect("entry exists")
                .mark_refreshing();
            ActionResult {
                response: Response::Output { output },
                refresh_repo_id: Some(repo_id.to_owned()),
                refresh_cwd: Some(stored_cwd),
                stop: false,
            }
        }
    }
}

/// エントリは必ず `process_query`（Query 経由）で生成される。
/// 未知の `repo_id` への Update（ブランチ切替直後の再導出 ID など）は無視する。
/// `cwd: PathBuf::new()` / `last_queried_at: None` の孤立エントリが生まれるのを防ぐ。
fn process_update(
    repo_id: &RepoId,
    output: &str,
    refresh_mode: RefreshMode,
    pr_outputs: &[PrOutput],
    entries: &mut HashMap<RepoId, CacheEntry>,
) -> ActionResult {
    if let Some(entry) = entries.get_mut(repo_id) {
        let pr_outputs = pr_outputs
            .iter()
            .map(|p| PrOutput {
                pr_id: p.pr_id,
                output: p.output.clone(),
            })
            .collect();
        entry.update(output.to_owned(), pr_outputs, refresh_mode);
    }
    ActionResult {
        response: Response::Ok,
        refresh_repo_id: None,
        refresh_cwd: None,
        stop: false,
    }
}
