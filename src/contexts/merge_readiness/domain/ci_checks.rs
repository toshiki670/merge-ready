use super::error::RepositoryError;

/// 個別チェックのバケット種別
///
/// `Fail`/`Cancel`/`ActionRequired` は `ci-fail`/`ci-action` 判定に使用する。
pub enum CheckBucket {
    Fail,
    Cancel,
    ActionRequired,
    Other,
}

/// チェック全体の集約状態
pub enum CiStatus {
    Fail,
    ActionRequired,
    Pass,
}

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

pub trait CiChecksRepository {
    /// # Errors
    /// Returns `RepositoryError` if the check buckets cannot be fetched.
    fn fetch_check_buckets(&self) -> Result<Vec<CheckBucket>, RepositoryError>;
}
