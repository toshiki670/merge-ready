/// ブランチとベースブランチの同期状態（インフラから取得した生の値）
pub enum BranchSyncStatus {
    Clean,
    Conflicting,
    Behind,
    /// 同期状態を判定できない（取得手段が利用不可）
    Unknown,
}

/// ブランチ同期のブロッカー評価状態
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BranchSyncState {
    Conflict,
    UpdateBranch,
    SyncUnknown,
}

/// ブランチ同期状態のドメインモデル
pub struct BranchSync {
    status: BranchSyncStatus,
}

impl BranchSync {
    #[must_use]
    pub fn new(status: BranchSyncStatus) -> Self {
        Self { status }
    }

    #[must_use]
    pub fn state(&self) -> Option<BranchSyncState> {
        match self.status {
            BranchSyncStatus::Conflicting => Some(BranchSyncState::Conflict),
            BranchSyncStatus::Behind => Some(BranchSyncState::UpdateBranch),
            BranchSyncStatus::Unknown => Some(BranchSyncState::SyncUnknown),
            BranchSyncStatus::Clean => None,
        }
    }
}
