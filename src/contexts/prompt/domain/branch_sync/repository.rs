use super::super::error::RepositoryError;
use super::BranchSync;

pub trait BranchSyncRepository {
    /// # Errors
    /// Returns `RepositoryError` if the sync status cannot be fetched.
    fn fetch_sync_status(&self) -> Result<BranchSync, RepositoryError>;
}
