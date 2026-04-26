/// ブランチ同期のブロッカー評価状態
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BranchSyncState {
    /// ベースブランチとのマージ競合が発生している
    Conflict,
    /// ベースブランチに対して遅れており更新が必要
    UpdateBranch,
    /// 同期状態を判定できない（Compare API が利用不可など）
    SyncUnknown,
}
