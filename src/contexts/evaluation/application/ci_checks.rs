use super::port::PromptStatusPort;
use crate::contexts::evaluation::domain::error::RepositoryError;
use crate::contexts::evaluation::domain::pr_state::blocked::ci::CiChecks;

/// CI チェック結果を取得する。失敗時は `Err` を返す（エラー表示は呼び出し元が担う）。
pub fn fetch(port: &impl PromptStatusPort) -> Result<CiChecks, RepositoryError> {
    port.fetch_checks()
}
