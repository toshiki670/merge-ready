use crate::contexts::prompt::domain::error::RepositoryError;

pub trait ErrorLogger {
    fn log(&self, msg: &str);
}

/// エラー時に表示するトークンの意味オブジェクト
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
            err_logger.log("rate limit");
            err_presenter.show_error(ErrorToken::RateLimited);
        }
        RepositoryError::Unexpected(msg) => {
            err_logger.log(&msg);
            err_presenter.show_error(ErrorToken::ApiError);
        }
    }
}
