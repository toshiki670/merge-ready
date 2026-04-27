use serde::Deserialize;

#[derive(Deserialize)]
pub(super) struct GhPrView {
    pub(super) state: String,
    #[serde(rename = "isDraft")]
    pub(super) is_draft: bool,
    pub(super) mergeable: String,
    #[serde(rename = "mergeStateStatus")]
    pub(super) merge_state_status: String,
    #[serde(rename = "reviewDecision")]
    pub(super) review_decision: Option<String>,
    #[serde(rename = "baseRefName", default)]
    pub(super) base_ref_name: String,
    #[serde(rename = "headRefName", default)]
    pub(super) head_ref_name: String,
}

#[derive(Deserialize)]
pub(super) struct GhCheckItem {
    pub(super) bucket: String,
}

#[derive(Deserialize)]
pub(super) struct GhRepoView {
    #[serde(rename = "nameWithOwner")]
    pub(super) name_with_owner: String,
}

#[derive(Deserialize)]
pub(super) struct GhCompare {
    pub(super) behind_by: u64,
}

#[derive(Deserialize)]
pub(super) struct GhDefaultBranchRef {
    pub(super) name: String,
}

#[derive(Deserialize)]
pub(super) struct GhRepoViewFull {
    #[serde(rename = "defaultBranchRef")]
    pub(super) default_branch_ref: GhDefaultBranchRef,
}

pub(super) enum CheckBucket {
    Fail,
    Cancel,
    ActionRequired,
    Pending,
    Other,
}

pub(super) fn translate_bucket(bucket: &str) -> CheckBucket {
    match bucket {
        "fail" => CheckBucket::Fail,
        "cancel" => CheckBucket::Cancel,
        "action_required" => CheckBucket::ActionRequired,
        "pending" => CheckBucket::Pending,
        _ => CheckBucket::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translate_bucket_pending() {
        assert!(matches!(translate_bucket("pending"), CheckBucket::Pending));
    }
}
