use crate::domain::review::{ReviewStatus, ReviewRepository};

/// レビュー状態を取得する。失敗時は `None` を返してエラー出力する。
pub fn fetch(repo: &impl ReviewRepository) -> Option<ReviewStatus> {
    match repo.fetch_review_status() {
        Ok(status) => Some(status),
        Err(e) => {
            super::errors::handle(e);
            None
        }
    }
}

/// レビュー状態を評価し、該当するトークンを返す
pub fn check(status: &ReviewStatus) -> Option<&'static str> {
    crate::domain::review::evaluate(status)
}
