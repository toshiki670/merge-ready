use crate::contexts::daemon::domain::cache::{CachePort, RefreshMode};

/// キャッシュを更新するユースケース
pub fn update(port: &impl CachePort, repo_id: &str, output: &str, refresh_mode: RefreshMode) {
    port.update(repo_id, output, refresh_mode);
}
