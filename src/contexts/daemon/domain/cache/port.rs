use super::RepoId;
use crate::shared::protocol::PrOutput;
use crate::shared::refresh_mode::RefreshMode;

/// キャッシュの更新ポート。
pub trait CachePort {
    /// キャッシュを更新する。失敗は静かに無視する。
    fn update(
        &self,
        repo_id: &RepoId,
        output: &str,
        refresh_mode: RefreshMode,
        pr_outputs: Vec<PrOutput>,
    ) -> impl std::future::Future<Output = ()> + Send;
}
