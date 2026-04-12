use crate::domain::error::RepositoryError;

/// レビュー決定状態
pub enum ReviewStatus {
    ChangesRequested,
    Other,
}

pub trait ReviewRepository {
    fn fetch_review_status(&self) -> Result<ReviewStatus, RepositoryError>;
}
