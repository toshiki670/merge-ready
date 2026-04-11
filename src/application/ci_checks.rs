use crate::domain;
use crate::domain::ci_checks::CheckBucket;
use crate::infra::pr_client::PrClient;

/// `gh pr checks` を呼び出し、[`CheckBucket`] のリストを返す。
///
/// 取得に失敗した場合は `None` を返し、エラー出力を行う。
pub fn fetch(client: &impl PrClient) -> Option<Vec<CheckBucket>> {
    match client.pr_checks() {
        Ok(buckets) => Some(buckets),
        Err(e) => {
            super::errors::handle(e);
            None
        }
    }
}

/// CI チェック結果を集約・評価し、該当するトークンを返す
pub fn check(buckets: &[CheckBucket]) -> Option<&'static str> {
    domain::ci_checks::evaluate(&domain::ci_checks::aggregate(buckets))
}
