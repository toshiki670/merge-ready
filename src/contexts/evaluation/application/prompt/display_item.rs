use crate::contexts::evaluation::domain::prompt::State;
use crate::contexts::evaluation::domain::prompt::pull_request::state::blocked::BlockedState;
use crate::contexts::evaluation::domain::prompt::pull_request::state::blocked::GenericBlockedState;
use crate::contexts::evaluation::domain::prompt::pull_request::state::blocked::branch_sync::BranchSyncState;
use crate::contexts::evaluation::domain::prompt::pull_request::state::blocked::ci::CiState;
use crate::contexts::evaluation::domain::prompt::pull_request::state::blocked::review::ReviewState;
use crate::contexts::evaluation::domain::prompt::pull_request::state::unblocked::UnblockedState;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contexts::evaluation::domain::prompt::pull_request::state::blocked::BlockedState;

    #[test]
    fn draft_maps_to_draft() {
        let items = from_state(State::Unblocked(UnblockedState::Draft));
        assert_eq!(items.len(), 1);
        assert!(matches!(items[0], DisplayItem::Draft));
    }

    #[test]
    fn merge_ready_maps_to_merge_ready() {
        let items = from_state(State::Unblocked(UnblockedState::MergeReady));
        assert_eq!(items.len(), 1);
        assert!(matches!(items[0], DisplayItem::MergeReady));
    }

    #[test]
    fn calculating_maps_to_status_calculating() {
        let items = from_state(State::Calculating);
        assert_eq!(items.len(), 1);
        assert!(matches!(items[0], DisplayItem::StatusCalculating));
    }

    #[test]
    fn conflict_maps_to_conflict() {
        let blocked = BlockedState {
            branch_sync: Some(BranchSyncState::Conflict),
            ci: None,
            review: None,
            generic: None,
        };
        let items = from_state(State::Blocked(blocked));
        assert_eq!(items.len(), 1);
        assert!(matches!(items[0], DisplayItem::Conflict));
    }

    #[test]
    fn multiple_blockers_produce_multiple_items() {
        let blocked = BlockedState {
            branch_sync: Some(BranchSyncState::Conflict),
            ci: Some(CiState::Fail),
            review: Some(ReviewState::ReviewRequired),
            generic: None,
        };
        let items = from_state(State::Blocked(blocked));
        assert_eq!(items.len(), 3);
        assert!(matches!(items[0], DisplayItem::Conflict));
        assert!(matches!(items[1], DisplayItem::CiFail));
        assert!(matches!(items[2], DisplayItem::ReviewRequired));
    }
}
