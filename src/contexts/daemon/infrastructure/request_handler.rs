use std::path::PathBuf;
use std::time::{Instant, SystemTime};

mod mapping;

use super::repo_id;
use crate::contexts::daemon::domain::cache::{
    CacheStore, Effect, QueryEvent, RefreshCompletedEvent, RepoId, on_query, on_refresh_completed,
};
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

pub(super) fn process(
    request: &Request,
    store: &mut CacheStore,
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
            process_query(&repo_id, branch, cwd_path, ttl, store, policy)
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
            store,
        ),
        Request::Stop => ActionResult {
            response: Response::Ok,
            refresh_repo_id: None,
            refresh_cwd: None,
            stop: true,
        },
        Request::Status => {
            let uptime_secs = started_at.elapsed().as_secs();
            let entry_count = store.entries().len();
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
            let dtos: Vec<EntryDto> = store.entries().values().flat_map(entry_to_dtos).collect();
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
    store: &mut CacheStore,
    policy: &RefreshPolicy,
) -> ActionResult {
    let now = Instant::now();
    let now_wall = SystemTime::now();
    let event = QueryEvent {
        cwd: cwd_path,
        branch,
        now,
        now_wall,
        stale_ttl: ttl,
    };
    let (new_store, effects) = on_query(std::mem::take(store), event, repo_id, policy);
    *store = new_store;
    effects_to_action_result(effects)
}

/// エントリは必ず `process_query`（Query 経由）で生成される。
/// 未知の `repo_id` への Update（ブランチ切替直後の再導出 ID など）は無視する。
fn process_update(
    repo_id: &RepoId,
    output: &str,
    refresh_mode: RefreshMode,
    pr_outputs: &[PrOutput],
    store: &mut CacheStore,
) -> ActionResult {
    let event = RefreshCompletedEvent {
        output: output.to_owned(),
        pr_outputs: pr_outputs.to_vec(),
        refresh_mode,
        now: Instant::now(),
        now_wall: SystemTime::now(),
    };
    let (new_store, _effects) = on_refresh_completed(std::mem::take(store), repo_id, event);
    *store = new_store;
    ActionResult {
        response: Response::Ok,
        refresh_repo_id: None,
        refresh_cwd: None,
        stop: false,
    }
}

fn effects_to_action_result(effects: Vec<Effect>) -> ActionResult {
    let mut response = Response::Output {
        output: String::new(),
    };
    let mut refresh_repo_id = None;
    let mut refresh_cwd = None;
    for e in effects {
        match e {
            Effect::EmitOutput(s) => {
                response = Response::Output { output: s };
            }
            Effect::SpawnRefresh { repo_id, cwd } => {
                refresh_repo_id = Some(repo_id);
                refresh_cwd = Some(cwd);
            }
            Effect::RecordExpired { .. } | Effect::EnterBackoff { .. } => {}
        }
    }
    ActionResult {
        response,
        refresh_repo_id,
        refresh_cwd,
        stop: false,
    }
}
