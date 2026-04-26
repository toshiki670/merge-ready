use super::branch_sync::{BranchSync, BranchSyncState};
use super::ci_checks::{CiChecks, CiState};
use super::review::{Review, ReviewState};
use super::unblocked::{MergeReadiness, UnblockedState};

pub struct PromptEvaluation<'a> {
    pub branch_sync: &'a BranchSync,
    pub ci_checks: &'a CiChecks,
    pub review: &'a Review,
    pub readiness: &'a MergeReadiness,
}

/// PR がブロックされているときのブロッカー集合（複数同時に存在できる）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockedState {
    pub branch_sync: Option<BranchSyncState>,
    pub ci: Option<CiState>,
    pub review: Option<ReviewState>,
}

/// PR の評価状態（排他的）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum PrState {
    /// PR 未作成（#156 で対応）
    NoPr,
    /// PR 作成済み・blocker あり
    Blocked(BlockedState),
    /// PR 作成済み・blocker なし
    Unblocked(UnblockedState),
    /// PR の対象外（何も表示しない）
    NotApplicable,
    /// 全パターン不一致の暫定状態（#157 解決後に廃止予定）
    Unknown,
}

pub struct PromptDecisionPolicy;

impl PromptDecisionPolicy {
    #[must_use]
    pub fn evaluate(input: &PromptEvaluation<'_>) -> PrState {
        let branch_sync = input.branch_sync.state();
        let ci = input.ci_checks.state();
        let review = input.review.state();

        if branch_sync.is_some() || ci.is_some() || review.is_some() {
            PrState::Blocked(BlockedState {
                branch_sync,
                ci,
                review,
            })
        } else if let Some(unblocked) = input.readiness.to_unblocked_state() {
            PrState::Unblocked(unblocked)
        } else {
            PrState::Unknown
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contexts::evaluation::domain::branch_sync::{BranchSync, BranchSyncStatus};
    use crate::contexts::evaluation::domain::ci_checks::{CheckBucket, CiChecks};
    use crate::contexts::evaluation::domain::review::{Review, ReviewStatus};
    use crate::contexts::evaluation::domain::unblocked::MergeReadiness;

    #[test]
    fn returns_merge_ready_when_no_blockers() {
        let state = PromptDecisionPolicy::evaluate(&PromptEvaluation {
            branch_sync: &BranchSync::new(BranchSyncStatus::Clean),
            ci_checks: &CiChecks::new(vec![CheckBucket::Other]),
            review: &Review::new(ReviewStatus::Approved),
            readiness: &MergeReadiness {
                is_draft: false,
                is_protected: true,
            },
        });

        assert!(matches!(
            state,
            PrState::Unblocked(UnblockedState::MergeReady)
        ));
    }

    #[test]
    fn returns_draft_when_draft_pr() {
        let state = PromptDecisionPolicy::evaluate(&PromptEvaluation {
            branch_sync: &BranchSync::new(BranchSyncStatus::Clean),
            ci_checks: &CiChecks::new(vec![]),
            review: &Review::new(ReviewStatus::NoDecision),
            readiness: &MergeReadiness {
                is_draft: true,
                is_protected: false,
            },
        });

        assert!(matches!(state, PrState::Unblocked(UnblockedState::Draft)));
    }

    #[test]
    fn returns_unknown_when_no_blockers_and_not_ready() {
        let state = PromptDecisionPolicy::evaluate(&PromptEvaluation {
            branch_sync: &BranchSync::new(BranchSyncStatus::Clean),
            ci_checks: &CiChecks::new(vec![]),
            review: &Review::new(ReviewStatus::NoDecision),
            readiness: &MergeReadiness {
                is_draft: false,
                is_protected: false,
            },
        });

        assert!(matches!(state, PrState::Unknown));
    }

    #[test]
    fn returns_blocked_with_all_blockers() {
        let state = PromptDecisionPolicy::evaluate(&PromptEvaluation {
            branch_sync: &BranchSync::new(BranchSyncStatus::Conflicting),
            ci_checks: &CiChecks::new(vec![CheckBucket::Fail]),
            review: &Review::new(ReviewStatus::ChangesRequested),
            readiness: &MergeReadiness {
                is_draft: false,
                is_protected: true,
            },
        });

        let PrState::Blocked(blocked) = state else {
            panic!("expected Blocked");
        };
        assert_eq!(blocked.branch_sync, Some(BranchSyncState::Conflict));
        assert_eq!(blocked.ci, Some(CiState::Fail));
        assert_eq!(blocked.review, Some(ReviewState::ChangesRequested));
    }

    #[test]
    fn returns_blocked_without_merge_ready_when_blockers_exist() {
        let state = PromptDecisionPolicy::evaluate(&PromptEvaluation {
            branch_sync: &BranchSync::new(BranchSyncStatus::Conflicting),
            ci_checks: &CiChecks::new(vec![CheckBucket::Fail]),
            review: &Review::new(ReviewStatus::ChangesRequested),
            readiness: &MergeReadiness {
                is_draft: false,
                is_protected: true,
            },
        });

        assert!(matches!(state, PrState::Blocked(_)));
    }
}
