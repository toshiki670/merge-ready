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

/// CI のブロッカー評価状態
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CiState {
    /// チェックが失敗またはキャンセルされている
    Fail,
    /// 手動アクションが必要なチェックが存在する
    ActionRequired,
    /// チェックが実行中
    Pending,
}

/// レビューのブロッカー評価状態
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewState {
    /// レビュアーが変更を要求している
    ChangesRequested,
    /// レビュアーがまだアサインされていない
    ReviewRequired,
}

/// 汎用ブロッカー評価状態
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenericBlockedState {
    /// API で原因を特定できないブロック（`mergeStateStatus == "BLOCKED"` かつ他シグナルすべて None）
    BlockedUnknown,
}

/// PR がブロックされているときのブロッカー集合（複数同時に存在できる）
///
/// 各フィールドは独立した blocker 評価状態を保持する。`None` はそのカテゴリに blocker がないことを示す。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockedState {
    /// ブランチ同期の blocker（競合・更新必要・判定不能）
    pub branch_sync: Option<BranchSyncState>,
    /// CI チェックの blocker（失敗・アクション必要）
    pub ci: Option<CiState>,
    /// レビューの blocker（変更要求）
    pub review: Option<ReviewState>,
    /// 汎用ブロッカー（API では原因を特定できないブロック）
    pub generic: Option<GenericBlockedState>,
}
