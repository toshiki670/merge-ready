use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

mod mapping;

use super::protocol::{EntryDto, PrOutputDto, Request, Response};
use super::repo_id;
use crate::contexts::daemon::domain::cache::{CacheEntry, RefreshMode, RepoId};
use crate::contexts::daemon::domain::refresh_policy::RefreshPolicy;
use mapping::{entry_to_dtos, pr_outputs_from_dtos};

pub(super) struct ActionResult {
    pub(super) response: Response,
    /// `Some(repo_id)` のとき、ロック解放後にリフレッシュを起動する
    pub(super) refresh_repo_id: Option<RepoId>,
    pub(super) refresh_cwd: Option<PathBuf>,
    pub(super) stop: bool,
    /// レスポンス返却後に自己再起動する（version mismatch 時）
    pub(super) restart_after_response: bool,
}

#[derive(Debug, Clone, Copy)]
enum RefreshState {
    NeedsRefresh { restart_after: bool },
    Refreshing { restart_after: bool },
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
        Request::Query {
            cwd,
            client_version,
        } => {
            let version_mismatch = client_version.as_str() != env!("CARGO_PKG_VERSION");

            let Some((repo_id_str, branch)) = repo_id::repo_info_from_cwd(cwd) else {
                return ActionResult {
                    response: Response::Output {
                        output: String::new(),
                    },
                    refresh_repo_id: None,
                    refresh_cwd: None,
                    stop: false,
                    restart_after_response: version_mismatch,
                };
            };

            let repo_id = RepoId::new(repo_id_str);
            let cwd_path = PathBuf::from(cwd);
            process_query(
                &repo_id,
                branch,
                cwd_path,
                ttl,
                version_mismatch,
                entries,
                policy,
            )
        }
        Request::Update {
            repo_id,
            output,
            refresh_mode,
            pr_outputs,
        } => process_update(
            &RepoId::new(repo_id.clone()),
            output,
            RefreshMode::from(*refresh_mode),
            pr_outputs,
            entries,
        ),
        Request::Stop => ActionResult {
            response: Response::Ok,
            refresh_repo_id: None,
            refresh_cwd: None,
            stop: true,
            restart_after_response: false,
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
                restart_after_response: false,
            }
        }
        Request::Entries => {
            let dtos: Vec<EntryDto> = entries.values().flat_map(entry_to_dtos).collect();
            ActionResult {
                response: Response::Entries { entries: dtos },
                refresh_repo_id: None,
                refresh_cwd: None,
                stop: false,
                restart_after_response: false,
            }
        }
    }
}

fn process_query(
    repo_id: &RepoId,
    branch: String,
    cwd_path: PathBuf,
    ttl: u64,
    restart_after_response: bool,
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
                restart_after_response,
            }
        }
        Some(entry) => {
            // Stale または TTL 超過
            let output = entry.output().to_owned();
            let has_fetched = entry.has_fetched();
            let stored_cwd = entry.cwd().to_path_buf();
            let refresh_state = if entry.is_refreshing() {
                RefreshState::Refreshing {
                    restart_after: restart_after_response,
                }
            } else {
                RefreshState::NeedsRefresh {
                    restart_after: restart_after_response,
                }
            };
            let is_terminal = entry.refresh_mode() == RefreshMode::Terminal;
            // Query を受けたので last_queried_at を更新し Cold カウンタをリセット
            let was_cold = entry.is_cold_or_never_queried(policy.warm_to_cold_secs);
            if was_cold {
                entry.reset_cold_count();
            }
            entry.record_query();
            if is_terminal && matches!(refresh_state, RefreshState::NeedsRefresh { .. }) {
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
                restart_after_response,
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
        (RefreshState::Refreshing { restart_after }, false) => ActionResult {
            response: Response::Output {
                output: "? loading".to_owned(),
            },
            refresh_repo_id: None,
            refresh_cwd: None,
            stop: false,
            restart_after_response: restart_after,
        },
        (RefreshState::Refreshing { restart_after }, true) => ActionResult {
            response: Response::Output { output },
            refresh_repo_id: None,
            refresh_cwd: None,
            stop: false,
            restart_after_response: restart_after,
        },
        (RefreshState::NeedsRefresh { restart_after }, _) => {
            entries
                .get_mut(repo_id)
                .expect("entry exists")
                .mark_refreshing();
            ActionResult {
                response: Response::Output { output },
                refresh_repo_id: Some(repo_id.to_owned()),
                refresh_cwd: Some(stored_cwd),
                stop: false,
                restart_after_response: restart_after,
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
    pr_outputs_dto: &[PrOutputDto],
    entries: &mut HashMap<RepoId, CacheEntry>,
) -> ActionResult {
    if let Some(entry) = entries.get_mut(repo_id) {
        let pr_outputs = pr_outputs_from_dtos(pr_outputs_dto);
        entry.update(output.to_owned(), pr_outputs, refresh_mode);
    }
    ActionResult {
        response: Response::Ok,
        refresh_repo_id: None,
        refresh_cwd: None,
        stop: false,
        restart_after_response: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contexts::daemon::domain::cache::PrOutput;

    fn make_entry(output: &str, refresh_mode: RefreshMode) -> CacheEntry {
        let mut e = CacheEntry::new(PathBuf::new(), String::new(), 5);
        e.update(output.to_owned(), vec![], refresh_mode);
        e.record_query();
        e
    }

    // ── is_active (via CacheEntry) ─────────────────────────────────────────────

    #[test]
    fn active_when_non_empty_output_and_warm() {
        assert!(make_entry("✓ Ready for merge", RefreshMode::Warm).is_active());
    }

    #[test]
    fn active_when_hot() {
        assert!(make_entry("⧖ Wait for CI", RefreshMode::Hot).is_active());
    }

    #[test]
    fn inactive_when_empty_output() {
        assert!(!make_entry("", RefreshMode::Warm).is_active());
    }

    #[test]
    fn inactive_when_terminal() {
        assert!(!make_entry("✓ Ready for merge", RefreshMode::Terminal).is_active());
    }

    // ── process_update ─────────────────────────────────────────────────────────

    #[test]
    fn process_update_sets_refresh_mode_terminal() {
        let repo_id = RepoId::new("repo");
        let mut entries = HashMap::new();
        entries.insert(
            repo_id.clone(),
            make_entry("✓ Ready for merge", RefreshMode::Warm),
        );
        process_update(&repo_id, "", RefreshMode::Terminal, &[], &mut entries);
        assert_eq!(entries[&repo_id].refresh_mode(), RefreshMode::Terminal);
    }

    #[test]
    fn process_update_sets_refresh_mode_hot() {
        let repo_id = RepoId::new("repo");
        let mut entries = HashMap::new();
        entries.insert(
            repo_id.clone(),
            make_entry("✓ Ready for merge", RefreshMode::Warm),
        );
        process_update(
            &repo_id,
            "⧖ Wait for CI",
            RefreshMode::Hot,
            &[],
            &mut entries,
        );
        assert_eq!(entries[&repo_id].refresh_mode(), RefreshMode::Hot);
    }

    #[test]
    fn process_update_stores_pr_outputs() {
        let repo_id = RepoId::new("repo");
        let mut entries = HashMap::new();
        entries.insert(
            repo_id.clone(),
            make_entry(
                "✓ Ready for merge #200 ✎ Ready for review #201",
                RefreshMode::Warm,
            ),
        );

        process_update(
            &repo_id,
            "✓ Ready for merge #200 ✎ Ready for review #201",
            RefreshMode::Warm,
            &[
                PrOutputDto {
                    pr_id: 200,
                    output: "✓ Ready for merge #200".to_owned(),
                },
                PrOutputDto {
                    pr_id: 201,
                    output: "✎ Ready for review #201".to_owned(),
                },
            ],
            &mut entries,
        );

        let pr_outputs = entries[&repo_id].pr_outputs();
        assert_eq!(pr_outputs.len(), 2);
        assert_eq!(pr_outputs[0].pr_id, 200);
        assert_eq!(pr_outputs[0].output, "✓ Ready for merge #200");
        assert_eq!(pr_outputs[1].pr_id, 201);
        assert_eq!(pr_outputs[1].output, "✎ Ready for review #201");
    }

    #[test]
    fn process_update_unknown_repo_id_is_ignored() {
        // ブランチ切替後に spawn_refresh が新 repo_id で Update してきた場合、
        // エントリを新規作成せず無視する（孤立エントリ防止）
        let mut entries = HashMap::new();
        process_update(
            &RepoId::new("unknown-repo"),
            "output",
            RefreshMode::Warm,
            &[],
            &mut entries,
        );
        assert!(
            entries.is_empty(),
            "未知の repo_id への Update はエントリを作成しないはず"
        );
    }

    #[test]
    fn process_update_clears_terminal_when_pr_reopens() {
        let repo_id = RepoId::new("repo");
        let mut entries = HashMap::new();
        entries.insert(repo_id.clone(), make_entry("", RefreshMode::Terminal));
        process_update(
            &repo_id,
            "✓ Ready for merge",
            RefreshMode::Warm,
            &[],
            &mut entries,
        );
        assert_eq!(entries[&repo_id].refresh_mode(), RefreshMode::Warm);
    }

    // ── entry_to_dtos ─────────────────────────────────────────────────────────

    #[test]
    fn entry_to_dtos_expands_pr_outputs() {
        let mut entry = CacheEntry::new(PathBuf::from("/repo"), "feat/multi".to_owned(), 5);
        entry.update(
            "✓ Ready for merge #200 ✎ Ready for review #201".to_owned(),
            vec![
                PrOutput {
                    pr_id: 200,
                    output: "✓ Ready for merge #200".to_owned(),
                },
                PrOutput {
                    pr_id: 201,
                    output: "✎ Ready for review #201".to_owned(),
                },
            ],
            RefreshMode::Warm,
        );

        let dtos = entry_to_dtos(&entry);

        assert_eq!(dtos.len(), 2);
        assert_eq!(dtos[0].cwd, "/repo");
        assert_eq!(dtos[0].branch, "feat/multi");
        assert_eq!(dtos[0].pr_id, Some(200));
        assert_eq!(dtos[0].output, "✓ Ready for merge #200");
        assert_eq!(dtos[1].cwd, "/repo");
        assert_eq!(dtos[1].branch, "feat/multi");
        assert_eq!(dtos[1].pr_id, Some(201));
        assert_eq!(dtos[1].output, "✎ Ready for review #201");
    }

    #[test]
    fn entry_to_dtos_keeps_aggregate_output_without_pr_outputs() {
        let mut entry = CacheEntry::new(PathBuf::from("/repo"), "chore/no-pr".to_owned(), 5);
        entry.update("+ Create PR".to_owned(), vec![], RefreshMode::Warm);

        let dtos = entry_to_dtos(&entry);

        assert_eq!(dtos.len(), 1);
        assert_eq!(dtos[0].cwd, "/repo");
        assert_eq!(dtos[0].branch, "chore/no-pr");
        assert_eq!(dtos[0].pr_id, None);
        assert_eq!(dtos[0].output, "+ Create PR");
    }
}
