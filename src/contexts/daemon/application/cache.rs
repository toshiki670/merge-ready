use crate::contexts::daemon::domain::cache::{CachePort, RefreshMode, RepoId};

/// キャッシュを更新するユースケース
pub fn update(port: &impl CachePort, repo_id: &RepoId, output: &str, refresh_mode: RefreshMode) {
    port.update(repo_id, output, refresh_mode);
}
