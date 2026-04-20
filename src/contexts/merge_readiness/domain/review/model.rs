use super::ReviewStatus;
use crate::contexts::merge_readiness::domain::signal::PromptSignal;

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
    pub fn signal(&self) -> Option<PromptSignal> {
        match self.status {
            ReviewStatus::ChangesRequested => Some(PromptSignal::ReviewRequested),
            ReviewStatus::Other => None,
        }
    }
}
