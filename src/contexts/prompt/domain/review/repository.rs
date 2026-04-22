use super::super::error::RepositoryError;
use super::Review;

pub trait ReviewRepository {
    /// # Errors
    /// Returns `RepositoryError` if the review state cannot be fetched.
    fn fetch_review(&self) -> Result<Review, RepositoryError>;
}
