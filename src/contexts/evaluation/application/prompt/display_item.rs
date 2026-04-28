use crate::contexts::evaluation::domain::pr_state::NotApplicableState;
use crate::contexts::evaluation::domain::pr_state::PrState;
use crate::contexts::evaluation::domain::pr_state::blocked::BlockedState;
use crate::contexts::evaluation::domain::pr_state::blocked::GenericBlockedState;
use crate::contexts::evaluation::domain::pr_state::blocked::branch_sync::BranchSyncState;
use crate::contexts::evaluation::domain::pr_state::blocked::ci::CiState;
use crate::contexts::evaluation::domain::pr_state::blocked::review::ReviewState;
use crate::contexts::evaluation::domain::pr_state::unblocked::UnblockedState;

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
    NoPullRequest,
    Draft,
    StatusCalculating,
    BlockedUnknown,
}

pub fn from_pr_state(state: PrState) -> Vec<DisplayItem> {
    match state {
        PrState::Blocked(blocked) => from_blocked(blocked),
        PrState::Unblocked(UnblockedState::MergeReady) => vec![DisplayItem::MergeReady],
        PrState::Unblocked(UnblockedState::Draft) => vec![DisplayItem::Draft],
        PrState::NoPr => vec![DisplayItem::NoPullRequest],
        PrState::NotApplicable(NotApplicableState::Calculating) => {
            vec![DisplayItem::StatusCalculating]
        }
        PrState::NotApplicable(_) | PrState::Unknown => vec![],
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
    use crate::contexts::evaluation::domain::pr_state::blocked::BlockedState;

    #[test]
    fn no_pr_maps_to_no_pull_request() {
        let items = from_pr_state(PrState::NoPr);
        assert_eq!(items.len(), 1);
        assert!(matches!(items[0], DisplayItem::NoPullRequest));
    }

    #[test]
    fn draft_maps_to_draft() {
        let items = from_pr_state(PrState::Unblocked(UnblockedState::Draft));
        assert_eq!(items.len(), 1);
        assert!(matches!(items[0], DisplayItem::Draft));
    }

    #[test]
    fn merge_ready_maps_to_merge_ready() {
        let items = from_pr_state(PrState::Unblocked(UnblockedState::MergeReady));
        assert_eq!(items.len(), 1);
        assert!(matches!(items[0], DisplayItem::MergeReady));
    }

    #[test]
    fn calculating_maps_to_status_calculating() {
        let items = from_pr_state(PrState::NotApplicable(NotApplicableState::Calculating));
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
        let items = from_pr_state(PrState::Blocked(blocked));
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
        let items = from_pr_state(PrState::Blocked(blocked));
        assert_eq!(items.len(), 3);
        assert!(matches!(items[0], DisplayItem::Conflict));
        assert!(matches!(items[1], DisplayItem::CiFail));
        assert!(matches!(items[2], DisplayItem::ReviewRequired));
    }

    #[test]
    fn not_applicable_non_calculating_produces_empty() {
        use crate::contexts::evaluation::domain::pr_state::NotApplicableState;
        let items = from_pr_state(PrState::NotApplicable(NotApplicableState::Merged));
        assert!(items.is_empty());
    }
}
