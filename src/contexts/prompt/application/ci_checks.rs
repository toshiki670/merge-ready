use crate::contexts::prompt::domain::ci_checks::{CiChecks, CiChecksRepository};
use crate::contexts::prompt::domain::error::RepositoryError;

/// CI チェック結果を取得する。失敗時は `Err` を返す（エラー表示は呼び出し元が担う）。
pub fn fetch(repo: &impl CiChecksRepository) -> Result<CiChecks, RepositoryError> {
    repo.fetch_checks()
}
