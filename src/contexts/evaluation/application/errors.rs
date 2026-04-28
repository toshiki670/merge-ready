use crate::contexts::evaluation::domain::error::RepositoryError;

pub use super::port::ErrorLogger;
use super::port::{ErrorCategory, LogRecord};

/// エラー時に表示するトークン。メッセージはエラー発生箇所で定義される。
#[derive(Clone)]
pub struct ErrorToken {
    pub message: String,
}

/// `RepositoryError` をエラートークンに変換する。`NotFound` は表示不要なため `None` を返す。
pub fn into_token<L: ErrorLogger>(e: RepositoryError, logger: &L) -> Option<ErrorToken> {
    match e {
        RepositoryError::Unauthenticated => Some(ErrorToken {
            message: "authentication required".to_owned(),
        }),
        RepositoryError::NotFound => None,
        RepositoryError::RateLimited => {
            logger.log(&LogRecord {
                category: ErrorCategory::RateLimit,
                detail: None,
            });
            Some(ErrorToken {
                message: "rate limited".to_owned(),
            })
        }
        RepositoryError::Unexpected => Some(ErrorToken {
            message: "unexpected error".to_owned(),
        }),
    }
}
