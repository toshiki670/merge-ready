pub mod blocked;
pub mod not_applicable;
pub mod unblocked;

use blocked::BlockedState;
use blocked::branch_sync::BranchSyncState;
use blocked::ci::CiState;
use blocked::review::ReviewState;
use unblocked::UnblockedState;

pub use not_applicable::NotApplicableState;

use super::error::RepositoryError;

/// PR の評価状態（排他的）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrState {
    /// PR 未作成
    NoPr,
    /// PR 作成済み・blocker あり
    Blocked(BlockedState),
    /// PR 作成済み・blocker なし
    Unblocked(UnblockedState),
    /// 評価対象外（理由を保持）
    NotApplicable(NotApplicableState),
    /// 全パターン不一致の暫定状態（#157 解決後に廃止予定）
    Unknown,
}

impl PrState {
    /// PR が終端状態（マージ済み・クローズ済み）かどうかを返す。
    ///
    /// daemon がポーリングを停止すべきかどうかの判定に使う。
    #[must_use]
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            PrState::NotApplicable(NotApplicableState::Merged | NotApplicableState::Closed)
        )
    }
}

pub trait PrRepository {
    /// # Errors
    /// Returns `RepositoryError` if the PR state cannot be fetched.
    fn fetch(&self) -> Result<PrState, RepositoryError>;
}

/// PR の評価状態を決定するビジネスルール
///
/// blocker が1つでもあれば `Blocked`、なければ `Unblocked`（`unblocked` の値による）、
/// それ以外は `Unknown`。
#[must_use]
pub fn evaluate(
    branch_sync: Option<BranchSyncState>,
    ci: Option<CiState>,
    review: Option<ReviewState>,
    unblocked: Option<UnblockedState>,
) -> PrState {
    if branch_sync.is_some() || ci.is_some() || review.is_some() {
        PrState::Blocked(BlockedState {
            branch_sync,
            ci,
            review,
        })
    } else if let Some(u) = unblocked {
        PrState::Unblocked(u)
    } else {
        PrState::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use blocked::branch_sync::BranchSyncState;
    use blocked::ci::CiState;
    use blocked::review::ReviewState;
    use unblocked::UnblockedState;

    #[test]
    fn returns_merge_ready_when_no_blockers() {
        let state = evaluate(None, None, None, Some(UnblockedState::MergeReady));
        assert!(matches!(
            state,
            PrState::Unblocked(UnblockedState::MergeReady)
        ));
    }

    #[test]
    fn returns_draft_when_draft_pr() {
        let state = evaluate(None, None, None, Some(UnblockedState::Draft));
        assert!(matches!(state, PrState::Unblocked(UnblockedState::Draft)));
    }

    #[test]
    fn returns_unknown_when_no_blockers_and_not_ready() {
        let state = evaluate(None, None, None, None);
        assert!(matches!(state, PrState::Unknown));
    }

    #[test]
    fn returns_blocked_with_all_blockers() {
        let state = evaluate(
            Some(BranchSyncState::Conflict),
            Some(CiState::Fail),
            Some(ReviewState::ChangesRequested),
            Some(UnblockedState::MergeReady),
        );
        let PrState::Blocked(blocked) = state else {
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
        assert!(matches!(state, PrState::Blocked(_)));
    }
}
