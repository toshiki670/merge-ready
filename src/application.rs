mod branch_sync;
mod ci_checks;
pub(super) mod errors;
mod merge_ready;
mod pr_state;
mod review;

use crate::domain::{
    branch_sync::BranchSyncRepository, ci_checks::CiChecksRepository,
    merge_ready::MergeReadinessRepository, pr_state::PrStateRepository,
    review::ReviewRepository,
};

/// PR マージ可否チェックのユースケース
pub fn run<C>(client: &C)
where
    C: PrStateRepository
        + BranchSyncRepository
        + CiChecksRepository
        + ReviewRepository
        + MergeReadinessRepository,
{
    let Some(lifecycle) = pr_state::fetch(client) else {
        return;
    };

    if !pr_state::is_open(&lifecycle) {
        return;
    }

    let Some(sync_status) = branch_sync::fetch(client) else {
        return;
    };
    let Some(buckets) = ci_checks::fetch(client) else {
        return;
    };
    let Some(review_status) = review::fetch(client) else {
        return;
    };
    let Some(readiness) = merge_ready::fetch(client) else {
        return;
    };

    let mut tokens: Vec<&'static str> = Vec::new();
    if let Some(t) = branch_sync::check(&sync_status) {
        tokens.push(t);
    }
    if let Some(t) = ci_checks::check(&buckets) {
        tokens.push(t);
    }
    if let Some(t) = review::check(&review_status) {
        tokens.push(t);
    }

    merge_ready::display(&readiness, &tokens);
}
