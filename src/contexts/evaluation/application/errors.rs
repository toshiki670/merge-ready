use crate::contexts::evaluation::domain::error::{ErrorCategory, LogRecord, RepositoryError};

pub trait ErrorLogger {
    fn log(&self, record: &LogRecord);
}

/// エラー時に表示するトークン。メッセージはエラー発生箇所で定義される。
#[derive(Clone)]
pub struct ErrorToken {
    pub message: String,
}

/// エラーをユーザーに表示するポート
pub trait ErrorPresenter {
    fn show_error(&self, token: ErrorToken);
}

/// `RepositoryError` を受け取り、エラーポリシーに従って出力・ログ記録を行う
pub fn handle(
    e: RepositoryError,
    err_logger: &impl ErrorLogger,
    err_presenter: &impl ErrorPresenter,
) {
    match e {
        RepositoryError::Unauthenticated => {
            err_presenter.show_error(ErrorToken {
                message: "authentication required".to_owned(),
            });
        }
        RepositoryError::NotFound => {}
        RepositoryError::RateLimited => {
            err_logger.log(&LogRecord {
                category: ErrorCategory::RateLimit,
                detail: None,
            });
            err_presenter.show_error(ErrorToken {
                message: "rate limited".to_owned(),
            });
        }
        RepositoryError::Unexpected(msg) => {
            let message = msg.lines().next().map(str::to_owned).unwrap_or_default();
            err_logger.log(&LogRecord {
                category: ErrorCategory::Unknown,
                detail: Some(msg),
            });
            err_presenter.show_error(ErrorToken { message });
        }
    }
}
