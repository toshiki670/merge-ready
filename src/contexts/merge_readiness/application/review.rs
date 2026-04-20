use super::errors::{ErrorLogger, ErrorPresenter};
use crate::contexts::merge_readiness::domain::review::{Review, ReviewRepository};

/// レビュー状態を取得する。失敗時は `None` を返してエラー出力する。
pub fn fetch(
    repo: &impl ReviewRepository,
    err_logger: &impl ErrorLogger,
    err_presenter: &impl ErrorPresenter,
) -> Option<Review> {
    match repo.fetch_review() {
        Ok(review) => Some(review),
        Err(e) => {
            super::errors::handle(e, err_logger, err_presenter);
            None
        }
    }
}
