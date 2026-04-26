/// PR がブロックされていないときの評価状態
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnblockedState {
    /// `is_draft=false` && `is_protected=true`
    MergeReady,
    /// `is_draft=true`（ready-for-review）
    Draft,
}
