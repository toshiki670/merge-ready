use crate::contexts::evaluation::domain::prompt::State;
use crate::contexts::evaluation::domain::prompt::pull_request::state::blocked::{
    BlockedState, BranchSyncState, CiState, GenericBlockedState, ReviewState,
};
use crate::contexts::evaluation::domain::prompt::pull_request::state::unblocked::UnblockedState;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DisplayItem {
    Conflict,
    UpdateBranch,
    SyncUnknown,
    CiFail,
    CiAction,
    CiPending,
    ChangesRequested,
    ReviewRequired,
    MergeReady,
    Draft,
    StatusCalculating,
    BlockedUnknown,
}

pub fn from_state(state: State) -> Vec<DisplayItem> {
    match state {
        State::Blocked(blocked) => from_blocked(blocked),
        State::Unblocked(UnblockedState::MergeReady) => vec![DisplayItem::MergeReady],
        State::Unblocked(UnblockedState::Draft) => vec![DisplayItem::Draft],
        State::Calculating => vec![DisplayItem::StatusCalculating],
    }
}

fn from_blocked(blocked: BlockedState) -> Vec<DisplayItem> {
    let mut items = Vec::new();
    if let Some(s) = blocked.branch_sync {
        items.push(match s {
            BranchSyncState::Conflict => DisplayItem::Conflict,
            BranchSyncState::UpdateBranch => DisplayItem::UpdateBranch,
            BranchSyncState::SyncUnknown => DisplayItem::SyncUnknown,
        });
    }
    if let Some(c) = blocked.ci {
        items.push(match c {
            CiState::Fail => DisplayItem::CiFail,
            CiState::ActionRequired => DisplayItem::CiAction,
            CiState::Pending => DisplayItem::CiPending,
        });
    }
    if let Some(r) = blocked.review {
        items.push(match r {
            ReviewState::ChangesRequested => DisplayItem::ChangesRequested,
            ReviewState::ReviewRequired => DisplayItem::ReviewRequired,
        });
    }
    if let Some(g) = blocked.generic {
        items.push(match g {
            GenericBlockedState::BlockedUnknown => DisplayItem::BlockedUnknown,
        });
    }
    items
}
