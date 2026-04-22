use super::errors::{ErrorLogger, ErrorPresenter};
use crate::contexts::prompt::domain::merge_ready::{MergeReadiness, MergeReadinessRepository};

/// マージ可否状態を取得する。失敗時は `None` を返してエラー出力する。
pub fn fetch(
    repo: &impl MergeReadinessRepository,
    err_logger: &impl ErrorLogger,
    err_presenter: &impl ErrorPresenter,
) -> Option<MergeReadiness> {
    match repo.fetch_readiness() {
        Ok(readiness) => Some(readiness),
        Err(e) => {
            super::errors::handle(e, err_logger, err_presenter);
            None
        }
    }
}
