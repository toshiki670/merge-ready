use super::errors::{ErrorLogger, ErrorPresenter};
use super::port::PromptStatusPort;
use crate::contexts::prompt::domain::merge_ready::MergeReadiness;

/// マージ可否状態を取得する。失敗時は `None` を返してエラー出力する。
pub fn fetch(
    port: &impl PromptStatusPort,
    err_logger: &impl ErrorLogger,
    err_presenter: &impl ErrorPresenter,
) -> Option<MergeReadiness> {
    match port.fetch_readiness() {
        Ok(readiness) => Some(readiness),
        Err(e) => {
            super::errors::handle(e, err_logger, err_presenter);
            None
        }
    }
}
