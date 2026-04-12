use crate::domain::error::RepositoryError;
use crate::{infra, presentation};

/// `RepositoryError` を受け取り、エラーポリシーに従って出力・ログ記録を行う
pub fn handle(e: RepositoryError) {
    match e {
        RepositoryError::NotInstalled | RepositoryError::AuthRequired => {
            presentation::display_error("! gh auth login");
        }
        RepositoryError::NoPr => {}
        RepositoryError::RateLimited => {
            infra::logger::append_error("rate limit");
            presentation::display_error("✗ rate-limited");
        }
        RepositoryError::ApiError(msg) => {
            infra::logger::append_error(&msg);
            presentation::display_error("✗ api-error");
        }
    }
}
