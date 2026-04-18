use super::OutputToken;
use super::errors::{ErrorLogger, ErrorPresenter};
use crate::contexts::merge_readiness::domain::review::{ReviewRepository, ReviewStatus};

/// レビュー状態を取得する。失敗時は `None` を返してエラー出力する。
pub fn fetch(
    repo: &impl ReviewRepository,
    err_logger: &impl ErrorLogger,
    err_presenter: &impl ErrorPresenter,
) -> Option<ReviewStatus> {
    match repo.fetch_review_status() {
        Ok(status) => Some(status),
        Err(e) => {
            super::errors::handle(e, err_logger, err_presenter);
            None
        }
    }
}

/// レビュー状態を評価し、該当するトークンを返す
pub fn check(status: &ReviewStatus) -> Option<OutputToken> {
    match status {
        ReviewStatus::ChangesRequested => Some(OutputToken::ReviewRequested),
        ReviewStatus::Other => None,
    }
}
