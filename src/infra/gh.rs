use std::io::ErrorKind;
use std::process::Command;

use serde::Deserialize;

use crate::domain::{
    branch_sync::BranchSyncStatus, ci_checks::CheckBucket, merge_ready::MergeReadiness,
    pr_state::PrLifecycle, review::ReviewStatus,
};
use crate::infra::pr_client::{PrClient, PrClientError, PrViewData};

// ── gh コマンドの生 JSON 構造（infra 内にのみ存在）──────────────────────────

#[derive(Deserialize)]
struct GhPrView {
    state: String,
    #[serde(rename = "isDraft")]
    is_draft: bool,
    mergeable: String,
    #[serde(rename = "mergeStateStatus")]
    merge_state_status: String,
    #[serde(rename = "reviewDecision")]
    review_decision: Option<String>,
}

#[derive(Deserialize)]
struct GhCheckItem {
    bucket: String,
    // `state` フィールドは JSON に必ず存在するが、判定には使用しない
    #[allow(dead_code)]
    state: String,
}

// ── GhClient ────────────────────────────────────────────────────────────────

pub struct GhClient;

impl PrClient for GhClient {
    fn pr_view(&self) -> Result<PrViewData, PrClientError> {
        let bytes = run_gh(&[
            "pr",
            "view",
            "--json",
            "state,isDraft,mergeable,mergeStateStatus,reviewDecision",
        ])?;
        let raw: GhPrView =
            serde_json::from_slice(&bytes).map_err(|e| PrClientError::ApiError(e.to_string()))?;
        Ok(PrViewData {
            lifecycle: translate_lifecycle(&raw.state),
            sync_status: translate_sync(&raw.mergeable, &raw.merge_state_status),
            review_status: translate_review(raw.review_decision.as_deref()),
            merge_readiness: translate_merge_readiness(raw.is_draft, &raw.merge_state_status),
        })
    }

    fn pr_checks(&self) -> Result<Vec<CheckBucket>, PrClientError> {
        let bytes = match run_gh(&["pr", "checks", "--json", "bucket,state"]) {
            Ok(b) => b,
            // CI が未設定のブランチではチェックなし（正常）として扱う
            Err(PrClientError::ApiError(msg)) if msg.contains("no checks reported") => {
                return Ok(vec![]);
            }
            Err(e) => return Err(e),
        };
        let items: Vec<GhCheckItem> =
            serde_json::from_slice(&bytes).map_err(|e| PrClientError::ApiError(e.to_string()))?;
        Ok(items.iter().map(|c| translate_bucket(&c.bucket)).collect())
    }
}

// ── 翻訳関数（gh 固有文字列 → domain enum）──────────────────────────────────

fn translate_lifecycle(state: &str) -> PrLifecycle {
    if state == "OPEN" {
        PrLifecycle::Open
    } else {
        PrLifecycle::NotOpen
    }
}

fn translate_sync(mergeable: &str, merge_state_status: &str) -> BranchSyncStatus {
    if mergeable == "CONFLICTING" {
        BranchSyncStatus::Conflicting
    } else if merge_state_status == "BEHIND" {
        BranchSyncStatus::Behind
    } else {
        BranchSyncStatus::Clean
    }
}

fn translate_review(decision: Option<&str>) -> ReviewStatus {
    if decision == Some("CHANGES_REQUESTED") {
        ReviewStatus::ChangesRequested
    } else {
        ReviewStatus::Other
    }
}

fn translate_merge_readiness(is_draft: bool, merge_state_status: &str) -> MergeReadiness {
    MergeReadiness {
        is_draft,
        is_protected: merge_state_status == "CLEAN" || merge_state_status == "HAS_HOOKS",
    }
}

fn translate_bucket(bucket: &str) -> CheckBucket {
    match bucket {
        "fail" => CheckBucket::Fail,
        "cancel" => CheckBucket::Cancel,
        "action_required" => CheckBucket::ActionRequired,
        _ => CheckBucket::Other,
    }
}

// ── gh コマンド実行・エラー判別 ──────────────────────────────────────────────

fn run_gh(args: &[&str]) -> Result<Vec<u8>, PrClientError> {
    let output = Command::new("gh").args(args).output();
    match output {
        Err(e) if e.kind() == ErrorKind::NotFound => Err(PrClientError::NotInstalled),
        Err(e) => Err(PrClientError::ApiError(e.to_string())),
        Ok(out) if out.status.success() => Ok(out.stdout),
        Ok(out) => {
            let exit_code = out.status.code().unwrap_or(1);
            let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
            Err(classify_error(exit_code, &stderr))
        }
    }
}

fn classify_error(exit_code: i32, stderr: &str) -> PrClientError {
    if exit_code == 4 || (exit_code == 1 && stderr.contains("HTTP 401")) {
        PrClientError::AuthRequired
    } else if exit_code == 1 && stderr.contains("no pull requests found") {
        PrClientError::NoPr
    } else if exit_code == 1 && stderr.contains("rate limit") {
        PrClientError::RateLimited
    } else {
        PrClientError::ApiError(stderr.to_owned())
    }
}
