use super::schema::CheckBucket;
use crate::contexts::evaluation::domain::prompt::pull_request::state::blocked::{
    BranchSyncState, CiState, ReviewState,
};
use crate::contexts::evaluation::domain::prompt::pull_request::state::unblocked::UnblockedState;

pub(super) fn translate_sync(mergeable: &str, behind_by: Option<u64>) -> Option<BranchSyncState> {
    match () {
        () if mergeable == "CONFLICTING" => Some(BranchSyncState::Conflict),
        () if matches!(behind_by, Some(0)) => None,
        () if behind_by.is_some() => Some(BranchSyncState::UpdateBranch),
        () => Some(BranchSyncState::SyncUnknown),
    }
}

pub(super) fn translate_review(decision: Option<&str>) -> Option<ReviewState> {
    match decision {
        Some("CHANGES_REQUESTED") => Some(ReviewState::ChangesRequested),
        Some("REVIEW_REQUIRED") => Some(ReviewState::ReviewRequired),
        _ => None,
    }
}

pub(super) fn translate_unblocked(
    is_draft: bool,
    merge_state_status: &str,
) -> Option<UnblockedState> {
    if is_draft {
        Some(UnblockedState::Draft)
    } else if merge_state_status == "CLEAN" || merge_state_status == "HAS_HOOKS" {
        Some(UnblockedState::MergeReady)
    } else {
        None
    }
}

pub(super) fn aggregate_ci(buckets: &[CheckBucket]) -> Option<CiState> {
    if buckets
        .iter()
        .any(|b| matches!(b, CheckBucket::Fail | CheckBucket::Cancel))
    {
        Some(CiState::Fail)
    } else if buckets
        .iter()
        .any(|b| matches!(b, CheckBucket::ActionRequired))
    {
        Some(CiState::ActionRequired)
    } else if buckets.iter().any(|b| matches!(b, CheckBucket::Pending)) {
        Some(CiState::Pending)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregate_ci_pending_returns_pending() {
        let buckets = vec![CheckBucket::Pending];
        assert_eq!(aggregate_ci(&buckets), Some(CiState::Pending));
    }

    #[test]
    fn aggregate_ci_fail_takes_priority_over_pending() {
        let buckets = vec![CheckBucket::Fail, CheckBucket::Pending];
        assert_eq!(aggregate_ci(&buckets), Some(CiState::Fail));
    }

    #[test]
    fn aggregate_ci_no_pending_returns_none() {
        let buckets = vec![CheckBucket::Other];
        assert_eq!(aggregate_ci(&buckets), None);
    }

    #[test]
    fn translate_sync_conflicting_takes_priority() {
        assert_eq!(
            translate_sync("CONFLICTING", Some(0)),
            Some(BranchSyncState::Conflict)
        );
    }

    #[test]
    fn translate_sync_zero_behind_is_clean() {
        assert_eq!(translate_sync("MERGEABLE", Some(0)), None);
    }

    #[test]
    fn translate_sync_unknown_without_compare_result() {
        assert_eq!(
            translate_sync("MERGEABLE", None),
            Some(BranchSyncState::SyncUnknown)
        );
    }

    #[test]
    fn translate_unblocked_merge_state_unknown_returns_none() {
        assert_eq!(translate_unblocked(false, "MERGE_STATE_UNKNOWN"), None);
    }

    #[test]
    fn translate_unblocked_unknown_returns_none() {
        assert_eq!(translate_unblocked(false, "UNKNOWN"), None);
    }

    #[test]
    fn translate_unblocked_blocked_returns_none() {
        assert_eq!(translate_unblocked(false, "BLOCKED"), None);
    }
}
