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
struct GhRepoView {
    #[serde(rename = "nameWithOwner")]
    name_with_owner: String,
}

#[derive(Deserialize)]
struct GhCompare {
    behind_by: u64,
}

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
    // compare API 用（フィールドが存在しない場合はデフォルト値を使用）
    #[serde(rename = "baseRefName", default)]
    base_ref_name: String,
    #[serde(rename = "headRefName", default)]
    head_ref_name: String,
}

#[derive(Deserialize)]
struct GhCheckItem {
    bucket: String,
    // `state` は JSON に存在するが判定に不要なため宣言しない（serde はデフォルトで無視する）
}

// ── GhClient ────────────────────────────────────────────────────────────────

pub struct GhClient;

impl PrClient for GhClient {
    fn pr_view(&self) -> Result<PrViewData, PrClientError> {
        let bytes = run_gh(&[
            "pr",
            "view",
            "--json",
            "state,isDraft,mergeable,mergeStateStatus,reviewDecision,baseRefName,headRefName",
        ])?;
        let raw: GhPrView =
            serde_json::from_slice(&bytes).map_err(|e| PrClientError::ApiError(e.to_string()))?;
        let behind_by = fetch_behind_by(&raw.base_ref_name, &raw.head_ref_name);
        Ok(PrViewData {
            lifecycle: translate_lifecycle(&raw.state),
            sync_status: translate_sync(&raw.mergeable, behind_by),
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

fn translate_sync(mergeable: &str, behind_by: u64) -> BranchSyncStatus {
    if mergeable == "CONFLICTING" {
        BranchSyncStatus::Conflicting
    } else if behind_by > 0 {
        BranchSyncStatus::Behind
    } else {
        BranchSyncStatus::Clean
    }
}

/// GitHub Compare API でベースブランチとの差分コミット数を取得する。
///
/// `base_ref` / `head_ref` が空（テスト用フィクスチャや旧形式 JSON）の場合は
/// API を呼ばずに `0` を返す。API 呼び出しに失敗した場合も `0` を返す（劣化動作）。
fn fetch_behind_by(base_ref: &str, head_ref: &str) -> u64 {
    if base_ref.is_empty() || head_ref.is_empty() {
        return 0;
    }

    let name_with_owner = match run_gh(&["repo", "view", "--json", "nameWithOwner"]) {
        Ok(bytes) => match serde_json::from_slice::<GhRepoView>(&bytes) {
            Ok(r) => r.name_with_owner,
            Err(_) => return 0,
        },
        Err(_) => return 0,
    };

    let path = format!("repos/{name_with_owner}/compare/{base_ref}...{head_ref}");

    match run_gh(&["api", &path]) {
        Ok(bytes) => serde_json::from_slice::<GhCompare>(&bytes)
            .map(|c| c.behind_by)
            .unwrap_or(0),
        Err(_) => 0,
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
