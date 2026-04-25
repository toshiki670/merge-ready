use super::super::error::RepositoryError;
use super::MergeReadiness;

pub trait MergeReadinessRepository {
    /// # Errors
    /// Returns `RepositoryError` if the merge readiness cannot be fetched.
    fn fetch_readiness(&self) -> Result<MergeReadiness, RepositoryError>;
}
