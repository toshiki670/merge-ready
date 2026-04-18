use super::OutputToken;
use crate::contexts::merge_readiness::domain::ci_checks::{
    CheckBucket, CiChecksRepository, CiStatus,
};
use crate::contexts::merge_readiness::domain::error::RepositoryError;

/// CI チェック結果を取得する。失敗時は `Err` を返す（エラー表示は呼び出し元が担う）。
pub fn fetch(repo: &impl CiChecksRepository) -> Result<Vec<CheckBucket>, RepositoryError> {
    repo.fetch_check_buckets()
}

/// CI チェック結果を集約・評価し、該当するトークンを返す
pub fn check(buckets: &[CheckBucket]) -> Option<OutputToken> {
    match crate::contexts::merge_readiness::domain::ci_checks::aggregate(buckets) {
        CiStatus::Fail => Some(OutputToken::CiFail),
        CiStatus::ActionRequired => Some(OutputToken::CiAction),
        CiStatus::Pass => None,
    }
}
