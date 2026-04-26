use crate::contexts::evaluation::domain::error::RepositoryError;

/// 個別チェックのバケット種別
pub enum CheckBucket {
    Fail,
    Cancel,
    ActionRequired,
    Other,
}

/// チェック全体の集約状態（インフラから取得した生の値）
enum CiStatus {
    Fail,
    ActionRequired,
    Pass,
}

/// CI のブロッカー評価状態
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CiState {
    Fail,
    ActionRequired,
}

fn aggregate(buckets: &[CheckBucket]) -> CiStatus {
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

pub trait CiChecksRepository {
    /// # Errors
    /// Returns `RepositoryError` if the CI checks cannot be fetched.
    fn fetch_checks(&self) -> Result<CiChecks, RepositoryError>;
}
