use crate::infra::pr_client::PrClientError;
use crate::{infra, presentation};

/// `PrClientError` を受け取り、エラーポリシーに従って出力・ログ記録を行う
pub fn handle(e: PrClientError) {
    match e {
        PrClientError::NotInstalled | PrClientError::AuthRequired => {
            presentation::display_error("! gh auth login");
        }
        PrClientError::NoPr => {}
        PrClientError::RateLimited => {
            infra::logger::append_error("rate limit");
            presentation::display_error("✗ rate-limited");
        }
        PrClientError::ApiError(msg) => {
            infra::logger::append_error(&msg);
            presentation::display_error("✗ api-error");
        }
    }
}
