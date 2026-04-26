use super::{ReviewState, ReviewStatus};

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
