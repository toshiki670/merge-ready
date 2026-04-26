use crate::contexts::evaluation::domain::error::RepositoryError;
use crate::contexts::evaluation::domain::pr_state::blocked::branch_sync::{
    BranchSync, BranchSyncRepository,
};

/// ブランチ同期状態を取得する。失敗時は `Err` を返す（エラー表示は呼び出し元が担う）。
pub fn fetch(repo: &impl BranchSyncRepository) -> Result<BranchSync, RepositoryError> {
    repo.fetch_sync_status()
}
