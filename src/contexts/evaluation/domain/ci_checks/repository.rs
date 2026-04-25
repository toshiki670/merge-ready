use super::super::error::RepositoryError;
use super::CiChecks;

pub trait CiChecksRepository {
    /// # Errors
    /// Returns `RepositoryError` if the CI checks cannot be fetched.
    fn fetch_checks(&self) -> Result<CiChecks, RepositoryError>;
}
