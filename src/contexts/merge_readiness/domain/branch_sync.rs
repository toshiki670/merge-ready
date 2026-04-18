use super::error::RepositoryError;

/// ブランチとベースブランチの同期状態
pub enum BranchSyncStatus {
    Clean,
    Conflicting,
    Behind,
    /// 同期状態を判定できない（取得手段が利用不可）
    Unknown,
}

pub trait BranchSyncRepository {
    /// # Errors
    /// Returns `RepositoryError` if the sync status cannot be fetched.
    fn fetch_sync_status(&self) -> Result<BranchSyncStatus, RepositoryError>;
}
