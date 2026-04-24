use super::port::PromptStatusPort;
use crate::contexts::prompt::domain::branch_sync::BranchSync;
use crate::contexts::prompt::domain::error::RepositoryError;

/// ブランチ同期状態を取得する。失敗時は `Err` を返す（エラー表示は呼び出し元が担う）。
pub fn fetch(port: &impl PromptStatusPort) -> Result<BranchSync, RepositoryError> {
    port.fetch_sync_status()
}
