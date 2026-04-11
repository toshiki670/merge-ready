use crate::domain;
use crate::infra::pr_client::PrViewData;
use crate::presentation;

/// 評価結果に応じてマージ可否を `stdout` に出力する
///
/// - ブロッカーがあれば各トークンをスペース区切りで出力
/// - ブロッカーなし かつ `merge_ready` 条件を満たせば `✓ merge-ready` を出力
/// - それ以外（`draft`・`pending` 等）は何も出力しない
pub fn display(data: &PrViewData, tokens: &[&str]) {
    if !tokens.is_empty() {
        presentation::display(tokens);
    } else if domain::merge_ready::is_ready(&data.merge_readiness) {
        presentation::display(&["✓ merge-ready"]);
    }
}
