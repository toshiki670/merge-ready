use super::{CheckBucket, CiStatus};

/// 複数チェックを集約（優先度: `Fail` > `ActionRequired` > `Pass`）
#[must_use]
pub fn aggregate(buckets: &[CheckBucket]) -> CiStatus {
    if buckets
        .iter()
        .any(|b| matches!(b, CheckBucket::Fail | CheckBucket::Cancel))
    {
        CiStatus::Fail
    } else if buckets
        .iter()
        .any(|b| matches!(b, CheckBucket::ActionRequired))
    {
        CiStatus::ActionRequired
    } else {
        CiStatus::Pass
    }
}
