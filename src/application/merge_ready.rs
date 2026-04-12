use crate::application::OutputToken;
use crate::application::errors::{ErrorLogger, ErrorPresenter};
use crate::domain::merge_ready::{MergeReadiness, MergeReadinessRepository};

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

/// マージ可否状態を評価し、該当するトークンを返す
pub fn check(readiness: &MergeReadiness) -> Option<OutputToken> {
    if crate::domain::merge_ready::is_ready(readiness) {
        Some(OutputToken::MergeReady)
    } else {
        None
    }
}
