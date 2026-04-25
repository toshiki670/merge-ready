use crate::contexts::evaluation::domain::signal::PromptSignal;

/// ブランチとベースブランチの同期状態
pub enum BranchSyncStatus {
    Clean,
    Conflicting,
    Behind,
    /// 同期状態を判定できない（取得手段が利用不可）
    Unknown,
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
    pub fn signal(&self) -> Option<PromptSignal> {
        match self.status {
            BranchSyncStatus::Conflicting => Some(PromptSignal::Conflict),
            BranchSyncStatus::Behind => Some(PromptSignal::UpdateBranch),
            BranchSyncStatus::Unknown => Some(PromptSignal::SyncUnknown),
            BranchSyncStatus::Clean => None,
        }
    }
}
