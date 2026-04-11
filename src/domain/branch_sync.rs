/// ブランチとベースブランチの同期状態
pub enum BranchSyncStatus {
    Clean,
    Conflicting,
    Behind,
}

/// `branch_sync` グループ評価（択一: `Conflicting` 優先）
pub fn evaluate(status: &BranchSyncStatus) -> Option<&'static str> {
    match status {
        BranchSyncStatus::Conflicting => Some("✗ conflict"),
        BranchSyncStatus::Behind => Some("✗ update-branch"),
        BranchSyncStatus::Clean => None,
    }
}
