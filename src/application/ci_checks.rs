use crate::domain::ci_checks::{CheckBucket, CiChecksRepository};

/// CI チェック結果を取得する。失敗時は `None` を返してエラー出力する。
pub fn fetch(repo: &impl CiChecksRepository) -> Option<Vec<CheckBucket>> {
    match repo.fetch_check_buckets() {
        Ok(buckets) => Some(buckets),
        Err(e) => {
            super::errors::handle(e);
            None
        }
    }
}

/// CI チェック結果を集約・評価し、該当するトークンを返す
pub fn check(buckets: &[CheckBucket]) -> Option<&'static str> {
    crate::domain::ci_checks::evaluate(&crate::domain::ci_checks::aggregate(buckets))
}
