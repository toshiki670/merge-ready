use crate::application::errors::{ErrorLogger, ErrorPresenter};
use crate::application::OutputToken;
use crate::domain::ci_checks::{CheckBucket, CiChecksRepository, CiStatus};

/// CI チェック結果を取得する。失敗時は `None` を返してエラー出力する。
pub fn fetch(
    repo: &impl CiChecksRepository,
    err_logger: &impl ErrorLogger,
    err_presenter: &impl ErrorPresenter,
) -> Option<Vec<CheckBucket>> {
    match repo.fetch_check_buckets() {
        Ok(buckets) => Some(buckets),
        Err(e) => {
            super::errors::handle(e, err_logger, err_presenter);
            None
        }
    }
}

/// CI チェック結果を集約・評価し、該当するトークンを返す
pub fn check(buckets: &[CheckBucket]) -> Option<OutputToken> {
    match crate::domain::ci_checks::aggregate(buckets) {
        CiStatus::Fail => Some(OutputToken::CiFail),
        CiStatus::ActionRequired => Some(OutputToken::CiAction),
        CiStatus::Pass => None,
    }
}
