use crate::application::errors::{ErrorLogger, ErrorPresenter};
use crate::application::OutputToken;
use crate::domain::branch_sync::{BranchSyncRepository, BranchSyncStatus};

/// ブランチ同期状態を取得する。失敗時は `None` を返してエラー出力する。
pub fn fetch(
    repo: &impl BranchSyncRepository,
    err_logger: &impl ErrorLogger,
    err_presenter: &impl ErrorPresenter,
) -> Option<BranchSyncStatus> {
    match repo.fetch_sync_status() {
        Ok(status) => Some(status),
        Err(e) => {
            super::errors::handle(e, err_logger, err_presenter);
            None
        }
    }
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
