pub mod branch_sync;
pub mod ci;
pub mod generic;
pub mod review;

use branch_sync::BranchSyncState;
use ci::CiState;
pub use generic::GenericBlockedState;
use review::ReviewState;

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
