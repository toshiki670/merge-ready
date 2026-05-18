use crate::contexts::evaluation::domain::error::RepositoryError;

/// エラー時に表示するトークン。メッセージはエラー発生箇所で定義される。
#[derive(Clone, Debug)]
pub struct ErrorToken {
    pub message: String,
}

/// `RepositoryError` をエラートークンに変換する。
pub fn into_token(e: RepositoryError) -> ErrorToken {
    match e {
        RepositoryError::Unauthenticated => ErrorToken {
            message: "authentication required".to_owned(),
        },
        RepositoryError::RateLimited => ErrorToken {
            message: "rate limited".to_owned(),
        },
        RepositoryError::Unexpected => ErrorToken {
            message: "unexpected error".to_owned(),
        },
    }
}
