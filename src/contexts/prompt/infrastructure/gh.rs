use std::io::{ErrorKind, Read};
use std::process::{Command, Stdio};
use std::sync::{OnceLock, mpsc};
use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::contexts::prompt::domain::branch_sync::{BranchSync, BranchSyncStatus};
use crate::contexts::prompt::domain::ci_checks::{CheckBucket, CiChecks};
use crate::contexts::prompt::domain::error::RepositoryError;
use crate::contexts::prompt::domain::merge_ready::MergeReadiness;
use crate::contexts::prompt::domain::pr_state::PrLifecycle;
use crate::contexts::prompt::domain::review::{Review, ReviewStatus};

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
    cwd: Option<std::path::PathBuf>,
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
            cwd: None,
        }
    }

    #[must_use]
    pub fn new_in(cwd: std::path::PathBuf) -> Self {
        Self {
            pr_view_cache: OnceLock::new(),
            cwd: Some(cwd),
        }
    }

    fn run_gh(&self, args: &[&str]) -> Result<Vec<u8>, RepositoryError> {
        run_gh(args, self.cwd.as_deref())
    }

    fn pr_view_cached(&self) -> Result<&GhPrView, RepositoryError> {
        if let Some(cached) = self.pr_view_cache.get() {
            return Ok(cached);
        }
        let bytes = self.run_gh(&[
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

impl GhClient {
    /// # Errors
    /// Returns `RepositoryError` if the PR lifecycle cannot be fetched.
    pub fn fetch_lifecycle(&self) -> Result<PrLifecycle, RepositoryError> {
        let raw = self.pr_view_cached()?;
        Ok(translate_lifecycle(&raw.state))
    }

    /// # Errors
    /// Returns `RepositoryError` if the sync status cannot be fetched.
    pub fn fetch_sync_status(&self) -> Result<BranchSync, RepositoryError> {
        let raw = self.pr_view_cached()?;
        let behind_by =
            fetch_behind_by(&raw.base_ref_name, &raw.head_ref_name, self.cwd.as_deref());
        Ok(BranchSync::new(translate_sync(&raw.mergeable, behind_by)))
    }

    /// # Errors
    /// Returns `RepositoryError` if the CI checks cannot be fetched.
    pub fn fetch_checks(&self) -> Result<CiChecks, RepositoryError> {
        let bytes = match self.run_gh(&["pr", "checks", "--json", "bucket,state"]) {
            Ok(b) => b,
            // CI が未設定のブランチではチェックなし（正常）として扱う
            Err(RepositoryError::Unexpected(msg)) if msg.contains("no checks reported") => {
                return Ok(CiChecks::new(vec![]));
            }
            Err(e) => return Err(e),
        };
        let items: Vec<GhCheckItem> = serde_json::from_slice(&bytes)
            .map_err(|e| RepositoryError::Unexpected(e.to_string()))?;
        Ok(CiChecks::new(
            items.iter().map(|c| translate_bucket(&c.bucket)).collect(),
        ))
    }

    /// # Errors
    /// Returns `RepositoryError` if the review state cannot be fetched.
    pub fn fetch_review(&self) -> Result<Review, RepositoryError> {
        let raw = self.pr_view_cached()?;
        Ok(Review::new(translate_review(
            raw.review_decision.as_deref(),
        )))
    }

    /// # Errors
    /// Returns `RepositoryError` if the merge readiness cannot be fetched.
    pub fn fetch_readiness(&self) -> Result<MergeReadiness, RepositoryError> {
        let raw = self.pr_view_cached()?;
        Ok(translate_merge_readiness(
            raw.is_draft,
            &raw.merge_state_status,
        ))
    }
}

// ── 翻訳関数（gh 固有文字列 → domain enum）──────────────────────────────────

fn translate_lifecycle(state: &str) -> PrLifecycle {
    match state {
        "OPEN" => PrLifecycle::Open,
        "MERGED" => PrLifecycle::Merged,
        _ => PrLifecycle::Closed,
    }
}

fn translate_sync(mergeable: &str, behind_by: Option<u64>) -> BranchSyncStatus {
    match () {
        // conflict は Compare API に依らず判定可能なので Unknown より優先
        () if mergeable == "CONFLICTING" => BranchSyncStatus::Conflicting,
        () if let Some(0) = behind_by => BranchSyncStatus::Clean,
        () if let Some(_) = behind_by => BranchSyncStatus::Behind,
        () => BranchSyncStatus::Unknown,
    }
}

/// GitHub Compare API でベースブランチとの差分コミット数を取得する。
///
/// `base_ref` / `head_ref` が空の場合（`baseRefName` / `headRefName` フィールドが
/// 存在しない旧形式 JSON 等）は `Some(0)` を返す（フィールドなし ≒ 追跡不要）。
/// Compare API 呼び出しに失敗した場合は `None` を返す。
/// 呼び出し元は `None` を `BranchSyncStatus::Unknown`（同期状態判定不能）として扱う。
fn fetch_behind_by(base_ref: &str, head_ref: &str, cwd: Option<&std::path::Path>) -> Option<u64> {
    if base_ref.is_empty() || head_ref.is_empty() {
        return Some(0);
    }

    let name_with_owner = match run_gh(&["repo", "view", "--json", "nameWithOwner"], cwd) {
        Ok(bytes) => match serde_json::from_slice::<GhRepoView>(&bytes) {
            Ok(r) => r.name_with_owner,
            Err(_) => return None,
        },
        Err(_) => return None,
    };

    let path = format!("repos/{name_with_owner}/compare/{base_ref}...{head_ref}");

    match run_gh(&["api", &path], cwd) {
        Ok(bytes) => serde_json::from_slice::<GhCompare>(&bytes)
            .map(|c| c.behind_by)
            .ok(),
        Err(_) => None,
    }
}

fn translate_review(decision: Option<&str>) -> ReviewStatus {
    match decision {
        Some("CHANGES_REQUESTED") => ReviewStatus::ChangesRequested,
        Some("APPROVED") => ReviewStatus::Approved,
        Some("REVIEW_REQUIRED") => ReviewStatus::ReviewRequired,
        _ => ReviewStatus::NoDecision,
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

fn gh_timeout() -> Duration {
    let secs = std::env::var("MERGE_READY_GH_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(30);
    Duration::from_secs(secs)
}

fn run_gh(args: &[&str], cwd: Option<&std::path::Path>) -> Result<Vec<u8>, RepositoryError> {
    let mut cmd = Command::new("gh");
    cmd.args(args);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }

    let mut child = match cmd.spawn() {
        Err(e) if e.kind() == ErrorKind::NotFound => return Err(GhError::NotInstalled.into()),
        Err(e) => return Err(GhError::ApiError(e.to_string()).into()),
        Ok(c) => c,
    };

    let mut stdout_pipe = child.stdout.take().expect("piped");
    let mut stderr_pipe = child.stderr.take().expect("piped");

    let (tx_out, rx_out) = mpsc::channel::<Vec<u8>>();
    let (tx_err, rx_err) = mpsc::channel::<Vec<u8>>();
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout_pipe.read_to_end(&mut buf);
        let _ = tx_out.send(buf);
    });
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stderr_pipe.read_to_end(&mut buf);
        let _ = tx_err.send(buf);
    });

    let deadline = Instant::now() + gh_timeout();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = rx_out.recv().unwrap_or_default();
                let stderr = rx_err.recv().unwrap_or_default();
                if status.success() {
                    return Ok(stdout);
                }
                let exit_code = status.code().unwrap_or(1);
                let stderr_str = String::from_utf8_lossy(&stderr).into_owned();
                return Err(classify_gh_error(exit_code, &stderr_str).into());
            }
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(GhError::ApiError("gh command timed out".to_string()).into());
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => return Err(GhError::ApiError(e.to_string()).into()),
        }
    }
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
