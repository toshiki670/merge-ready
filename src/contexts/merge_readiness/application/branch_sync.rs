use crate::contexts::merge_readiness::domain::branch_sync::{BranchSync, BranchSyncRepository};
use crate::contexts::merge_readiness::domain::error::RepositoryError;

/// ブランチ同期状態を取得する。失敗時は `Err` を返す（エラー表示は呼び出し元が担う）。
pub fn fetch(repo: &impl BranchSyncRepository) -> Result<BranchSync, RepositoryError> {
    repo.fetch_sync_status()
}
