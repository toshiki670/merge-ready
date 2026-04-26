/// ブランチ同期のブロッカー評価状態
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BranchSyncState {
    Conflict,
    UpdateBranch,
    SyncUnknown,
}
