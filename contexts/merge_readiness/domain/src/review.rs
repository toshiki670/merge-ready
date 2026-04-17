use crate::error::RepositoryError;

/// レビュー決定状態
pub enum ReviewStatus {
    ChangesRequested,
    Other,
}

pub trait ReviewRepository {
    /// # Errors
    /// Returns `RepositoryError` if the review status cannot be fetched.
    fn fetch_review_status(&self) -> Result<ReviewStatus, RepositoryError>;
}
