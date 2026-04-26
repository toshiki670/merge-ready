use crate::contexts::evaluation::domain::error::RepositoryError;

/// レビュー決定状態（インフラから取得した生の値）
pub enum ReviewStatus {
    ChangesRequested,
    Approved,
    ReviewRequired,
    NoDecision,
}

/// レビューのブロッカー評価状態
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewState {
    ChangesRequested,
}

/// レビュー状態のドメインモデル
pub struct Review {
    status: ReviewStatus,
}

impl Review {
    #[must_use]
    pub fn new(status: ReviewStatus) -> Self {
        Self { status }
    }

    #[must_use]
    pub fn state(&self) -> Option<ReviewState> {
        match self.status {
            ReviewStatus::ChangesRequested => Some(ReviewState::ChangesRequested),
            ReviewStatus::Approved | ReviewStatus::ReviewRequired | ReviewStatus::NoDecision => {
                None
            }
        }
    }
}

pub trait ReviewRepository {
    /// # Errors
    /// Returns `RepositoryError` if the review status cannot be fetched.
    fn fetch_review(&self) -> Result<Review, RepositoryError>;
}
