use crate::contexts::daemon::domain::cache::{CachePort, PrOutput, RefreshMode, RepoId};

/// キャッシュを更新するユースケース
pub fn update(
    port: &impl CachePort,
    repo_id: &RepoId,
    output: &str,
    refresh_mode: RefreshMode,
    pr_outputs: Vec<PrOutput>,
) {
    port.update(repo_id, output, refresh_mode, pr_outputs);
}
