use crate::domain::merge_ready::{MergeReadiness, MergeReadinessRepository};
use crate::presentation;

/// マージ可否状態を取得する。失敗時は `None` を返してエラー出力する。
pub fn fetch(repo: &impl MergeReadinessRepository) -> Option<MergeReadiness> {
    match repo.fetch_readiness() {
        Ok(readiness) => Some(readiness),
        Err(e) => {
            super::errors::handle(e);
            None
        }
    }
}

/// 評価結果に応じてマージ可否を `stdout` に出力する
///
/// - ブロッカーがあれば各トークンをスペース区切りで出力
/// - ブロッカーなし かつ `merge_ready` 条件を満たせば `✓ merge-ready` を出力
/// - それ以外（`draft`・`pending` 等）は何も出力しない
pub fn display(readiness: &MergeReadiness, tokens: &[&str]) {
    if !tokens.is_empty() {
        presentation::display(tokens);
    } else if crate::domain::merge_ready::is_ready(readiness) {
        presentation::display(&["✓ merge-ready"]);
    }
}
