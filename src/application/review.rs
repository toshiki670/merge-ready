use crate::domain;
use crate::infra::pr_client::PrViewData;

/// レビュー状態を評価し、該当するトークンを返す
pub fn check(data: &PrViewData) -> Option<&'static str> {
    domain::review::evaluate(&data.review_status)
}
