use crate::contexts::prompt::domain::error::RepositoryError;

/// ログ記録のためのエラーカテゴリ（原因調査パス）
#[allow(dead_code)]
pub enum ErrorCategory {
    Auth,
    RateLimit,
    Timeout,
    Unknown,
}

/// ログに記録する構造化エントリ（原因調査パス）
pub struct LogRecord {
    pub category: ErrorCategory,
    /// `Unknown` 系のみ raw メッセージを保持する
    pub detail: Option<String>,
}

pub trait ErrorLogger {
    fn log(&self, record: &LogRecord);
}

/// エラー時に表示するトークンの意味オブジェクト（検知パス）
///
/// 文字列表現への変換は presentation 層が担う。
#[derive(Clone, Copy)]
pub enum ErrorToken {
    /// 認証が必要（ツール未インストール含む）
    AuthRequired,
    /// レート制限によりアクセス不可
    RateLimited,
    /// 予期しない API エラー
    ApiError,
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
            err_presenter.show_error(ErrorToken::AuthRequired);
        }
        RepositoryError::NotFound => {}
        RepositoryError::RateLimited => {
            err_logger.log(&LogRecord {
                category: ErrorCategory::RateLimit,
                detail: None,
            });
            err_presenter.show_error(ErrorToken::RateLimited);
        }
        RepositoryError::Unexpected(msg) => {
            err_logger.log(&LogRecord {
                category: ErrorCategory::Unknown,
                detail: Some(msg),
            });
            err_presenter.show_error(ErrorToken::ApiError);
        }
    }
}
