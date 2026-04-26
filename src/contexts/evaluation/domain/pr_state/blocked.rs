pub mod branch_sync;
pub mod ci;
pub mod review;

use branch_sync::BranchSyncState;
use ci::CiState;
use review::ReviewState;

/// PR がブロックされているときのブロッカー集合（複数同時に存在できる）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockedState {
    pub branch_sync: Option<BranchSyncState>,
    pub ci: Option<CiState>,
    pub review: Option<ReviewState>,
}
