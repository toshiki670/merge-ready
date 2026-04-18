use std::io::ErrorKind;
use std::process::Command;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::contexts::merge_readiness::domain::branch_sync::{
    BranchSyncRepository, BranchSyncStatus,
};
use crate::contexts::merge_readiness::domain::ci_checks::{CheckBucket, CiChecksRepository};
use crate::contexts::merge_readiness::domain::error::RepositoryError;
use crate::contexts::merge_readiness::domain::merge_ready::{
    MergeReadiness, MergeReadinessRepository,
};
use crate::contexts::merge_readiness::domain::pr_state::{PrLifecycle, PrStateRepository};
use crate::contexts::merge_readiness::domain::review::{ReviewRepository, ReviewStatus};

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

#[derive(Deserialize)]
struct GhRepoView {
    #[serde(rename = "nameWithOwner")]
    name_with_owner: String,
}

#[derive(Deserialize)]
struct GhCompare {
    behind_by: u64,
}

// ── GhClient ────────────────────────────────────────────────────────────────

pub struct GhClient {
    /// `gh pr view` の結果をキャッシュする（複数トレイト実装によるコマンド多重実行を防ぐ）
    ///
    /// `OnceLock` を使用してスレッド間でのキャッシュ共有を安全にする（並列フェッチ対応）。
    pr_view_cache: OnceLock<GhPrView>,
}

impl Default for GhClient {
    fn default() -> Self {
        Self::new()
    }
}

impl GhClient {
    #[must_use]
    pub fn new() -> Self {
        Self {
            pr_view_cache: OnceLock::new(),
        }
    }

    fn pr_view_cached(&self) -> Result<&GhPrView, RepositoryError> {
        if let Some(cached) = self.pr_view_cache.get() {
            return Ok(cached);
        }
        let bytes = run_gh(&[
            "pr",
            "view",
            "--json",
            "state,isDraft,mergeable,mergeStateStatus,reviewDecision,baseRefName,headRefName",
        ])?;
        let raw: GhPrView = serde_json::from_slice(&bytes)
            .map_err(|e| RepositoryError::Unexpected(e.to_string()))?;
        let _ = self.pr_view_cache.set(raw);
        Ok(self.pr_view_cache.get().expect("just set"))
    }
}

impl PrStateRepository for GhClient {
    fn fetch_lifecycle(&self) -> Result<PrLifecycle, RepositoryError> {
        let raw = self.pr_view_cached()?;
        Ok(translate_lifecycle(&raw.state))
    }
}

impl BranchSyncRepository for GhClient {
    fn fetch_sync_status(&self) -> Result<BranchSyncStatus, RepositoryError> {
        let raw = self.pr_view_cached()?;
        let behind_by = fetch_behind_by(&raw.base_ref_name, &raw.head_ref_name);
        Ok(translate_sync(&raw.mergeable, behind_by))
    }
}

impl CiChecksRepository for GhClient {
    fn fetch_check_buckets(&self) -> Result<Vec<CheckBucket>, RepositoryError> {
        let bytes = match run_gh(&["pr", "checks", "--json", "bucket,state"]) {
            Ok(b) => b,
            // CI が未設定のブランチではチェックなし（正常）として扱う
            Err(RepositoryError::Unexpected(msg)) if msg.contains("no checks reported") => {
                return Ok(vec![]);
            }
            Err(e) => return Err(e),
        };
        let items: Vec<GhCheckItem> = serde_json::from_slice(&bytes)
            .map_err(|e| RepositoryError::Unexpected(e.to_string()))?;
        Ok(items.iter().map(|c| translate_bucket(&c.bucket)).collect())
    }
}

impl ReviewRepository for GhClient {
    fn fetch_review_status(&self) -> Result<ReviewStatus, RepositoryError> {
        let raw = self.pr_view_cached()?;
        Ok(translate_review(raw.review_decision.as_deref()))
    }
}

impl MergeReadinessRepository for GhClient {
    fn fetch_readiness(&self) -> Result<MergeReadiness, RepositoryError> {
        let raw = self.pr_view_cached()?;
        Ok(translate_merge_readiness(
            raw.is_draft,
            &raw.merge_state_status,
        ))
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

fn translate_sync(mergeable: &str, behind_by: Option<u64>) -> BranchSyncStatus {
    if mergeable == "CONFLICTING" {
        // conflict は Compare API に依らず判定可能なので Unknown より優先
        BranchSyncStatus::Conflicting
    } else {
        match behind_by {
            None => BranchSyncStatus::Unknown,
            Some(0) => BranchSyncStatus::Clean,
            Some(_) => BranchSyncStatus::Behind,
        }
    }
}

/// GitHub Compare API でベースブランチとの差分コミット数を取得する。
///
/// `base_ref` / `head_ref` が空の場合（`baseRefName` / `headRefName` フィールドが
/// 存在しない旧形式 JSON 等）は `Some(0)` を返す（フィールドなし ≒ 追跡不要）。
/// Compare API 呼び出しに失敗した場合は `None` を返す。
/// 呼び出し元は `None` を `BranchSyncStatus::Unknown`（同期状態判定不能）として扱う。
fn fetch_behind_by(base_ref: &str, head_ref: &str) -> Option<u64> {
    if base_ref.is_empty() || head_ref.is_empty() {
        return Some(0);
    }

    let name_with_owner = match run_gh(&["repo", "view", "--json", "nameWithOwner"]) {
        Ok(bytes) => match serde_json::from_slice::<GhRepoView>(&bytes) {
            Ok(r) => r.name_with_owner,
            Err(_) => return None,
        },
        Err(_) => return None,
    };

    let path = format!("repos/{name_with_owner}/compare/{base_ref}...{head_ref}");

    match run_gh(&["api", &path]) {
        Ok(bytes) => serde_json::from_slice::<GhCompare>(&bytes)
            .map(|c| c.behind_by)
            .ok(),
        Err(_) => None,
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

/// gh CLI 固有のエラー種別（infra 内にのみ存在）
enum GhError {
    /// gh バイナリが見つからない
    NotInstalled,
    /// 認証エラー（exit 4 / HTTP 401）
    AuthRequired,
    /// 対象 PR が存在しない
    NoPr,
    /// API レート制限
    RateLimited,
    /// その他の CLI エラー
    ApiError(String),
}

impl From<GhError> for RepositoryError {
    fn from(e: GhError) -> Self {
        match e {
            GhError::NotInstalled | GhError::AuthRequired => RepositoryError::Unauthenticated,
            GhError::NoPr => RepositoryError::NotFound,
            GhError::RateLimited => RepositoryError::RateLimited,
            GhError::ApiError(msg) => RepositoryError::Unexpected(msg),
        }
    }
}

fn run_gh(args: &[&str]) -> Result<Vec<u8>, RepositoryError> {
    let output = Command::new("gh").args(args).output();
    let gh_err = match output {
        Err(e) if e.kind() == ErrorKind::NotFound => GhError::NotInstalled,
        Err(e) => GhError::ApiError(e.to_string()),
        Ok(out) if out.status.success() => return Ok(out.stdout),
        Ok(out) => {
            let exit_code = out.status.code().unwrap_or(1);
            let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
            classify_gh_error(exit_code, &stderr)
        }
    };
    Err(gh_err.into())
}

fn classify_gh_error(exit_code: i32, stderr: &str) -> GhError {
    if exit_code == 4 || (exit_code == 1 && stderr.contains("HTTP 401")) {
        GhError::AuthRequired
    } else if exit_code == 1 && stderr.contains("no pull requests found") {
        GhError::NoPr
    } else if exit_code == 1 && stderr.contains("rate limit") {
        GhError::RateLimited
    } else {
        GhError::ApiError(stderr.to_owned())
    }
}
