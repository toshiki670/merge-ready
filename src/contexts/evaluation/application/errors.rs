use crate::contexts::evaluation::domain::error::RepositoryError;

pub use super::port::ErrorLogger;
use super::port::{ErrorCategory, LogRecord};

/// エラー時に表示するトークン。メッセージはエラー発生箇所で定義される。
#[derive(Clone, Debug)]
pub struct ErrorToken {
    pub message: String,
}

/// `RepositoryError` をエラートークンに変換する。
pub fn into_token<L: ErrorLogger>(e: RepositoryError, logger: &L) -> ErrorToken {
    match e {
        RepositoryError::Unauthenticated => ErrorToken {
            message: "authentication required".to_owned(),
        },
        RepositoryError::RateLimited => {
            logger.log(&LogRecord {
                category: ErrorCategory::RateLimit,
                detail: None,
            });
            ErrorToken {
                message: "rate limited".to_owned(),
            }
        }
        RepositoryError::Unexpected => ErrorToken {
            message: "unexpected error".to_owned(),
        },
    }
}
