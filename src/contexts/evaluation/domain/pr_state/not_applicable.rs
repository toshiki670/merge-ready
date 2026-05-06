/// PR の評価が対象外となる理由
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotApplicableState {
    // is_terminal() / entries_are_terminal() の判定とテストで使用する。
    // gh pr list --state open では返されないが、将来の拡張のために保持している。
    #[allow(dead_code)]
    /// PR がマージ済み
    Merged,
    #[allow(dead_code)]
    /// PR がクローズ済み（マージなし）
    Closed,
    /// GitHub がマージ状態を計算中（過渡的な状態）
    Calculating,
}
