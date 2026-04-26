use super::status::CiState;
use super::{CheckBucket, CiStatus, aggregate::aggregate};

/// CI チェック集合のドメインモデル
pub struct CiChecks {
    buckets: Vec<CheckBucket>,
}

impl CiChecks {
    #[must_use]
    pub fn new(buckets: Vec<CheckBucket>) -> Self {
        Self { buckets }
    }

    #[must_use]
    pub fn state(&self) -> Option<CiState> {
        match aggregate(&self.buckets) {
            CiStatus::Fail => Some(CiState::Fail),
            CiStatus::ActionRequired => Some(CiState::ActionRequired),
            CiStatus::Pass => None,
        }
    }
}
