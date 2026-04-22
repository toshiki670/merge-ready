use super::{CheckBucket, CiStatus, aggregate::aggregate};
use crate::contexts::prompt::domain::signal::PromptSignal;

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
    pub fn signal(&self) -> Option<PromptSignal> {
        match aggregate(&self.buckets) {
            CiStatus::Fail => Some(PromptSignal::CiFail),
            CiStatus::ActionRequired => Some(PromptSignal::CiAction),
            CiStatus::Pass => None,
        }
    }
}
