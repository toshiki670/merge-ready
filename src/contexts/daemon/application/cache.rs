use crate::contexts::daemon::domain::cache::CachePort;

/// キャッシュを更新するユースケース
pub fn update(port: &impl CachePort, repo_id: &str, output: &str) {
    port.update(repo_id, output);
}
