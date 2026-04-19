use crate::contexts::merge_readiness::application::prompt::PromptEffect;
use crate::contexts::merge_readiness::domain::cache::{CachePort, CacheState};
use crate::contexts::status_cache::application::cache::{self, CacheQueryResult};
use crate::contexts::status_cache::infrastructure::daemon_client::DaemonClient;

use crate::InfraRepoIdPort;

struct DaemonCacheAdapter;

impl CachePort for DaemonCacheAdapter {
    fn check(&self, repo_id: &str) -> CacheState {
        match cache::query(&DaemonClient, repo_id) {
            CacheQueryResult::Fresh(s) => CacheState::Fresh(s),
            CacheQueryResult::Stale(s) => CacheState::Stale(s),   // daemon が refresh 予約済み
            // Miss: daemon が refresh 予約済み。Unavailable: lazy_start 済み → next call でヒット
            CacheQueryResult::Miss | CacheQueryResult::Unavailable => CacheState::Miss,
        }
    }
}

/// daemon 経由でキャッシュを参照して表示する。
///
/// daemon 未起動時は lazy start を試み "? loading" を表示する（daemon 起動後の次回 call でヒット）。
pub fn run() {
    let cache = DaemonCacheAdapter;
    match crate::contexts::merge_readiness::application::prompt::resolve_cached(
        &InfraRepoIdPort,
        &cache,
    ) {
        PromptEffect::NoOutput => {}
        PromptEffect::Show(s) | PromptEffect::ShowAndRefresh(s) => print!("{s}"),
        PromptEffect::ShowLoadingAndRefresh => print!("? loading"),
    }
}
