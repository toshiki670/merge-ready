use super::branch_sync::BranchSync;
use super::ci_checks::CiChecks;
use super::merge_ready::MergeReadiness;
use super::review::Review;
use super::signal::PromptSignal;

pub struct PromptEvaluation<'a> {
    pub branch_sync: &'a BranchSync,
    pub ci_checks: &'a CiChecks,
    pub review: &'a Review,
    pub readiness: &'a MergeReadiness,
}

pub struct PromptDecisionPolicy;

impl PromptDecisionPolicy {
    #[must_use]
    pub fn evaluate(input: &PromptEvaluation<'_>) -> Vec<PromptSignal> {
        let mut signals = Vec::new();

        if let Some(signal) = input.branch_sync.signal() {
            let _ = signals.push_mut(signal);
        }
        if let Some(signal) = input.ci_checks.signal() {
            let _ = signals.push_mut(signal);
        }
        if let Some(signal) = input.review.signal() {
            let _ = signals.push_mut(signal);
        }

        if signals.is_empty()
            && let Some(signal) = input.readiness.signal()
        {
            let _ = signals.push_mut(signal);
        }

        signals
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn includes_merge_ready_when_no_blockers() {
        let signals = PromptDecisionPolicy::evaluate(&PromptEvaluation {
            branch_sync: &BranchSync::new(super::super::branch_sync::BranchSyncStatus::Clean),
            ci_checks: &CiChecks::new(vec![super::super::ci_checks::CheckBucket::Other]),
            review: &Review::new(super::super::review::ReviewStatus::Approved),
            readiness: &MergeReadiness {
                is_draft: false,
                is_protected: true,
            },
        });

        assert_eq!(signals, vec![PromptSignal::MergeReady]);
    }

    #[test]
    fn does_not_include_merge_ready_when_blockers_exist() {
        let signals = PromptDecisionPolicy::evaluate(&PromptEvaluation {
            branch_sync: &BranchSync::new(super::super::branch_sync::BranchSyncStatus::Conflicting),
            ci_checks: &CiChecks::new(vec![super::super::ci_checks::CheckBucket::Fail]),
            review: &Review::new(super::super::review::ReviewStatus::ChangesRequested),
            readiness: &MergeReadiness {
                is_draft: false,
                is_protected: true,
            },
        });

        assert_eq!(
            signals,
            vec![
                PromptSignal::Conflict,
                PromptSignal::CiFail,
                PromptSignal::ReviewRequested
            ]
        );
    }
}
