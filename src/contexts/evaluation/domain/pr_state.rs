pub mod blocked;
pub mod not_applicable;
pub mod unblocked;

use blocked::BlockedState;
use blocked::GenericBlockedState;
use blocked::branch_sync::BranchSyncState;
use blocked::ci::CiState;
use blocked::review::ReviewState;
use unblocked::UnblockedState;

pub use not_applicable::NotApplicableState;

use super::error::RepositoryError;

/// PR 番号を表す値オブジェクト。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PrId(u64);

impl PrId {
    #[must_use]
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    #[must_use]
    pub fn as_u64(self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for PrId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// PR 番号と評価状態のペア。
pub struct PrEntry {
    pub pr_id: PrId,
    pub state: PrState,
}

/// PR エントリ群が全て終端状態（マージ済み・クローズ済み）かどうかを返す。
///
/// 空リストは「PRが後から作成される可能性がある」ため false。
#[must_use]
pub fn entries_are_terminal(entries: &[PrEntry]) -> bool {
    !entries.is_empty() && entries.iter().all(|e| e.state.is_terminal())
}

/// PR の評価状態（排他的）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrState {
    /// PR 作成済み・blocker あり
    Blocked(BlockedState),
    /// PR 作成済み・blocker なし
    Unblocked(UnblockedState),
    /// 評価対象外（理由を保持）
    NotApplicable(NotApplicableState),
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
    fn fetch(&self) -> Result<Vec<PrEntry>, RepositoryError>;
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
) -> PrState {
    if branch_sync.is_some() || ci.is_some() || review.is_some() {
        PrState::Blocked(BlockedState {
            branch_sync,
            ci,
            review,
            generic: None,
        })
    } else if let Some(u) = unblocked {
        PrState::Unblocked(u)
    } else {
        PrState::Blocked(BlockedState {
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
    use blocked::GenericBlockedState;
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
    fn returns_blocked_unknown_when_no_blockers_and_not_ready() {
        let state = evaluate(None, None, None, None);
        let PrState::Blocked(blocked) = state else {
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

    #[test]
    fn calculating_is_not_terminal() {
        let state = PrState::NotApplicable(NotApplicableState::Calculating);
        assert!(!state.is_terminal());
    }

    #[test]
    fn evaluate_sets_generic_none_when_blockers_present() {
        let state = evaluate(Some(BranchSyncState::Conflict), None, None, None);
        let PrState::Blocked(blocked) = state else {
            panic!("expected Blocked");
        };
        assert_eq!(blocked.generic, None);
    }

    // ── PrId ────────────────────────────────────────────────────────────────

    #[test]
    fn pr_id_display() {
        assert_eq!(PrId::new(42).to_string(), "42");
    }

    #[test]
    fn pr_id_as_u64() {
        assert_eq!(PrId::new(200).as_u64(), 200);
    }

    // ── entries_are_terminal ────────────────────────────────────────────────

    #[test]
    fn empty_entries_are_not_terminal() {
        assert!(!entries_are_terminal(&[]));
    }

    #[test]
    fn all_merged_entries_are_terminal() {
        let entries = vec![
            PrEntry {
                pr_id: PrId::new(1),
                state: PrState::NotApplicable(NotApplicableState::Merged),
            },
            PrEntry {
                pr_id: PrId::new(2),
                state: PrState::NotApplicable(NotApplicableState::Closed),
            },
        ];
        assert!(entries_are_terminal(&entries));
    }

    #[test]
    fn mixed_entries_are_not_terminal() {
        let entries = vec![
            PrEntry {
                pr_id: PrId::new(1),
                state: PrState::NotApplicable(NotApplicableState::Merged),
            },
            PrEntry {
                pr_id: PrId::new(2),
                state: PrState::Unblocked(UnblockedState::MergeReady),
            },
        ];
        assert!(!entries_are_terminal(&entries));
    }
}
