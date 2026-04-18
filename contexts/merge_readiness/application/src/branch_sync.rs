use crate::OutputToken;
use merge_readiness_domain::branch_sync::{BranchSyncRepository, BranchSyncStatus};
use merge_readiness_domain::error::RepositoryError;

/// ブランチ同期状態を取得する。失敗時は `Err` を返す（エラー表示は呼び出し元が担う）。
pub fn fetch(repo: &impl BranchSyncRepository) -> Result<BranchSyncStatus, RepositoryError> {
    repo.fetch_sync_status()
}

/// ブランチ同期状態を評価し、該当するトークンを返す
pub fn check(status: &BranchSyncStatus) -> Option<OutputToken> {
    match status {
        BranchSyncStatus::Conflicting => Some(OutputToken::Conflict),
        BranchSyncStatus::Behind => Some(OutputToken::UpdateBranch),
        BranchSyncStatus::Unknown => Some(OutputToken::SyncUnknown),
        BranchSyncStatus::Clean => None,
    }
}
