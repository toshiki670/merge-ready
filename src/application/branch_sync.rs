use crate::domain::branch_sync::{BranchSyncStatus, BranchSyncRepository};

/// ブランチ同期状態を取得する。失敗時は `None` を返してエラー出力する。
pub fn fetch(repo: &impl BranchSyncRepository) -> Option<BranchSyncStatus> {
    match repo.fetch_sync_status() {
        Ok(status) => Some(status),
        Err(e) => {
            super::errors::handle(e);
            None
        }
    }
}

/// ブランチ同期状態を評価し、該当するトークンを返す
pub fn check(status: &BranchSyncStatus) -> Option<&'static str> {
    crate::domain::branch_sync::evaluate(status)
}
