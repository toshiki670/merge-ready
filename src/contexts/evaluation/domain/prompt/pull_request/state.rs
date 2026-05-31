pub mod blocked;
pub mod unblocked;

use blocked::{BlockedState, BranchSyncState, CiState, GenericBlockedState, ReviewState};
use unblocked::UnblockedState;

/// PR の評価状態（排他的）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// PR 作成済み・blocker あり
    Blocked(BlockedState),
    /// PR 作成済み・blocker なし
    Unblocked(UnblockedState),
    /// GitHub がマージ可能性を計算中（Blocked/Unblocked と排他的）
    Calculating,
}

/// PR の評価状態を決定するビジネスルール
///
/// blocker が1つでもあれば `Blocked`、なければ `Unblocked`（`unblocked` の値による）、
/// それ以外は blocker 不明として `Blocked(BlockedUnknown)`。
#[must_use]
pub fn evaluate(
    branch_sync: Option<BranchSyncState>,
    ci: Option<CiState>,
    review: Option<ReviewState>,
    unblocked: Option<UnblockedState>,
) -> State {
    if branch_sync.is_some() || ci.is_some() || review.is_some() {
        State::Blocked(BlockedState {
            branch_sync,
            ci,
            review,
            generic: None,
        })
    } else if let Some(u) = unblocked {
        State::Unblocked(u)
    } else {
        State::Blocked(BlockedState {
            branch_sync: None,
            ci: None,
            review: None,
            generic: Some(GenericBlockedState::BlockedUnknown),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use blocked::{BranchSyncState, CiState, GenericBlockedState, ReviewState};
    use std::assert_matches;
    use unblocked::UnblockedState;

    #[test]
    fn returns_merge_ready_when_no_blockers() {
        let state = evaluate(None, None, None, Some(UnblockedState::MergeReady));
        assert_matches!(state, State::Unblocked(UnblockedState::MergeReady));
    }

    #[test]
    fn returns_draft_when_draft_pr() {
        let state = evaluate(None, None, None, Some(UnblockedState::Draft));
        assert_matches!(state, State::Unblocked(UnblockedState::Draft));
    }

    #[test]
    fn returns_blocked_unknown_when_no_blockers_and_not_ready() {
        let state = evaluate(None, None, None, None);
        let State::Blocked(blocked) = state else {
            panic!("expected Blocked");
        };
        assert_eq!(blocked.generic, Some(GenericBlockedState::BlockedUnknown));
        assert!(blocked.branch_sync.is_none());
        assert!(blocked.ci.is_none());
        assert!(blocked.review.is_none());
    }

    #[test]
    fn returns_blocked_with_all_blockers() {
        let state = evaluate(
            Some(BranchSyncState::Conflict),
            Some(CiState::Fail),
            Some(ReviewState::ChangesRequested),
            Some(UnblockedState::MergeReady),
        );
        let State::Blocked(blocked) = state else {
            panic!("expected Blocked");
        };
        assert_eq!(blocked.branch_sync, Some(BranchSyncState::Conflict));
        assert_eq!(blocked.ci, Some(CiState::Fail));
        assert_eq!(blocked.review, Some(ReviewState::ChangesRequested));
    }

    #[test]
    fn returns_blocked_when_only_sync_blocker() {
        let state = evaluate(
            Some(BranchSyncState::UpdateBranch),
            None,
            None,
            Some(UnblockedState::MergeReady),
        );
        assert_matches!(state, State::Blocked(_));
    }

    #[test]
    fn evaluate_sets_generic_none_when_blockers_present() {
        let state = evaluate(Some(BranchSyncState::Conflict), None, None, None);
        let State::Blocked(blocked) = state else {
            panic!("expected Blocked");
        };
        assert_eq!(blocked.generic, None);
    }
}
