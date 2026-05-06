/// PR がブロックされていないときの評価状態
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnblockedState {
    /// マージ可能な状態（`is_draft=false` かつブランチ保護ルールを全て満たしている）
    MergeReady,
    /// ドラフト PR（`is_draft=true`）
    Draft,
}
