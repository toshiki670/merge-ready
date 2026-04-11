use crate::domain;
use crate::infra::pr_client::PrViewData;

/// ブランチ同期状態を評価し、該当するトークンを返す
pub fn check(data: &PrViewData) -> Option<&'static str> {
    domain::branch_sync::evaluate(&data.sync_status)
}
