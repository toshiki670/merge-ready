use super::errors::{ErrorLogger, ErrorPresenter};
use crate::contexts::prompt::domain::pr_state::{PrLifecycle, PrStateRepository};

/// ライフサイクル状態を取得する。失敗時は `None` を返してエラー出力する。
pub fn fetch(
    repo: &impl PrStateRepository,
    err_logger: &impl ErrorLogger,
    err_presenter: &impl ErrorPresenter,
) -> Option<PrLifecycle> {
    match repo.fetch_lifecycle() {
        Ok(lifecycle) => Some(lifecycle),
        Err(e) => {
            super::errors::handle(e, err_logger, err_presenter);
            None
        }
    }
}
