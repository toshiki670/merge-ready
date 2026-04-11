mod branch_sync;
mod ci_checks;
pub(super) mod errors;
mod merge_ready;
mod pr_state;
mod review;

use crate::infra::pr_client::PrClient;

/// PR マージ可否チェックのユースケース
pub fn run(client: &impl PrClient) {
    let Some(data) = pr_state::fetch(client) else {
        return;
    };

    if !pr_state::is_open(&data) {
        return;
    }

    let Some(buckets) = ci_checks::fetch(client) else {
        return;
    };

    let mut tokens: Vec<&'static str> = Vec::new();
    if let Some(t) = branch_sync::check(&data) {
        tokens.push(t);
    }
    if let Some(t) = ci_checks::check(&buckets) {
        tokens.push(t);
    }
    if let Some(t) = review::check(&data) {
        tokens.push(t);
    }

    merge_ready::display(&data, &tokens);
}
