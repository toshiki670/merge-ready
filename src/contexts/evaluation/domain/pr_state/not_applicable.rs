/// PR の評価が対象外となる理由
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotApplicableState {
    /// PR がマージ済み
    Merged,
    /// PR がクローズ済み（マージなし）
    Closed,
    /// main / master など、PR を作る対象でないブランチでの実行
    DefaultBranch,
    /// Git リポジトリ外での実行
    NoRepository,
    /// GitHub がマージ状態を計算中（過渡的な状態）
    Calculating,
}
