mod schema;

use std::io::{ErrorKind, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use schema::{
    CheckBucket, GhCheckItem, GhCompare, GhPrView, GhRepoView, GhRepoViewFull, translate_bucket,
};

use crate::contexts::evaluation::application::port::{ErrorCategory, ErrorLogger, LogRecord};
use crate::contexts::evaluation::domain::error::RepositoryError;
use crate::contexts::evaluation::domain::pr_state::blocked::GenericBlockedState;
use crate::contexts::evaluation::domain::pr_state::blocked::branch_sync::BranchSyncState;
use crate::contexts::evaluation::domain::pr_state::blocked::ci::CiState;
use crate::contexts::evaluation::domain::pr_state::blocked::review::ReviewState;
use crate::contexts::evaluation::domain::pr_state::not_applicable::NotApplicableState;
use crate::contexts::evaluation::domain::pr_state::unblocked::UnblockedState;
use crate::contexts::evaluation::domain::pr_state::{PrRepository, PrState, evaluate};
use crate::contexts::evaluation::infrastructure::git::{current_branch, is_git_repo};

// ── GhClient ────────────────────────────────────────────────────────────────

pub struct GhClient<L> {
    cwd: Option<std::path::PathBuf>,
    logger: L,
}

impl<L: ErrorLogger + Sync> GhClient<L> {
    #[must_use]
    pub fn new_in(cwd: std::path::PathBuf, logger: L) -> Self {
        Self {
            cwd: Some(cwd),
            logger,
        }
    }

    fn run_gh(&self, args: &[&str]) -> Result<Vec<u8>, GhError> {
        run_gh(args, self.cwd.as_deref())
    }

    fn log_and_convert(&self, e: GhError) -> RepositoryError {
        if let GhError::ApiError(ref msg) = e {
            self.logger.log(&LogRecord {
                category: ErrorCategory::Unknown,
                detail: Some(msg.clone()),
            });
        }
        RepositoryError::from(e)
    }

    fn fetch_pr_view(&self) -> Result<GhPrView, RepositoryError> {
        let bytes = self
            .run_gh(&[
                "pr",
                "view",
                "--json",
                "state,isDraft,mergeable,mergeStateStatus,reviewDecision,baseRefName,headRefName",
            ])
            .map_err(|e| self.log_and_convert(e))?;
        serde_json::from_slice(&bytes).map_err(|e| {
            self.logger.log(&LogRecord {
                category: ErrorCategory::Unknown,
                detail: Some(e.to_string()),
            });
            RepositoryError::Unexpected
        })
    }

    fn fetch_ci_state(&self) -> Result<Option<CiState>, RepositoryError> {
        let bytes = match self.run_gh(&["pr", "checks", "--json", "bucket,state"]) {
            Ok(b) => b,
            Err(GhError::ApiError(msg)) if msg.contains("no checks reported") => {
                return Ok(None);
            }
            Err(e) => return Err(self.log_and_convert(e)),
        };
        let items: Vec<GhCheckItem> = serde_json::from_slice(&bytes).map_err(|e| {
            self.logger.log(&LogRecord {
                category: ErrorCategory::Unknown,
                detail: Some(e.to_string()),
            });
            RepositoryError::Unexpected
        })?;
        let buckets: Vec<CheckBucket> = items.iter().map(|c| translate_bucket(&c.bucket)).collect();
        Ok(aggregate_ci(&buckets))
    }

    fn resolve_no_pr(&self) -> PrState {
        if self.is_default_branch() {
            PrState::NotApplicable(NotApplicableState::DefaultBranch)
        } else {
            PrState::NoPr
        }
    }

    fn is_default_branch(&self) -> bool {
        let Some(current) = current_branch(self.cwd.as_deref()) else {
            return false;
        };
        let Some(default) = self.default_branch() else {
            return false;
        };
        current == default
    }

    fn default_branch(&self) -> Option<String> {
        let bytes = run_gh(
            &["repo", "view", "--json", "defaultBranchRef"],
            self.cwd.as_deref(),
        )
        .ok()?;
        let repo: GhRepoViewFull = serde_json::from_slice(&bytes).ok()?;
        Some(repo.default_branch_ref.name)
    }
}

// ── PrRepository 実装 ────────────────────────────────────────────────────────

impl<L: ErrorLogger + Sync> PrRepository for GhClient<L> {
    fn fetch(&self) -> Result<PrState, RepositoryError> {
        if !is_git_repo(self.cwd.as_deref()) {
            return Ok(PrState::NotApplicable(NotApplicableState::NoRepository));
        }

        let pr_view = match self.fetch_pr_view() {
            Ok(v) => v,
            Err(RepositoryError::NotFound) => return Ok(self.resolve_no_pr()),
            Err(e) => return Err(e),
        };

        match pr_view.state.as_str() {
            "MERGED" => return Ok(PrState::NotApplicable(NotApplicableState::Merged)),
            s if s != "OPEN" => return Ok(PrState::NotApplicable(NotApplicableState::Closed)),
            _ => {}
        }

        // branch_sync は Compare API（追加 gh 呼び出し）、ci は pr checks（別 gh 呼び出し）なので並列化
        let (branch_sync, ci_result) = std::thread::scope(|s| {
            let cwd = self.cwd.as_deref();
            let base = pr_view.base_ref_name.as_str();
            let head = pr_view.head_ref_name.as_str();
            let mergeable = pr_view.mergeable.as_str();

            let sync_handle = s.spawn(move || {
                let behind_by = fetch_behind_by(base, head, cwd);
                translate_sync(mergeable, behind_by)
            });
            let ci_handle = s.spawn(|| self.fetch_ci_state());

            (
                sync_handle.join().expect("sync thread panicked"),
                ci_handle.join().expect("ci thread panicked"),
            )
        });

        let ci = ci_result?;
        let review = translate_review(pr_view.review_decision.as_deref());
        let unblocked = translate_unblocked(pr_view.is_draft, &pr_view.merge_state_status);

        let state = evaluate(branch_sync, ci, review, unblocked);
        if matches!(state, PrState::Unknown) {
            return Ok(match pr_view.merge_state_status.as_str() {
                "MERGE_STATE_UNKNOWN" | "UNKNOWN" => {
                    PrState::NotApplicable(NotApplicableState::Calculating)
                }
                "BLOCKED" => PrState::Blocked(
                    crate::contexts::evaluation::domain::pr_state::blocked::BlockedState {
                        branch_sync: None,
                        ci: None,
                        review: None,
                        generic: Some(GenericBlockedState::BlockedUnknown),
                    },
                ),
                _ => state,
            });
        }
        Ok(state)
    }
}

// ── 翻訳関数（gh 固有文字列 → domain enum）──────────────────────────────────

fn translate_sync(mergeable: &str, behind_by: Option<u64>) -> Option<BranchSyncState> {
    match () {
        () if mergeable == "CONFLICTING" => Some(BranchSyncState::Conflict),
        () if matches!(behind_by, Some(0)) => None,
        () if behind_by.is_some() => Some(BranchSyncState::UpdateBranch),
        () => Some(BranchSyncState::SyncUnknown),
    }
}

fn translate_review(decision: Option<&str>) -> Option<ReviewState> {
    match decision {
        Some("CHANGES_REQUESTED") => Some(ReviewState::ChangesRequested),
        Some("REVIEW_REQUIRED") => Some(ReviewState::ReviewRequired),
        _ => None,
    }
}

fn translate_unblocked(is_draft: bool, merge_state_status: &str) -> Option<UnblockedState> {
    if is_draft {
        Some(UnblockedState::Draft)
    } else if merge_state_status == "CLEAN" || merge_state_status == "HAS_HOOKS" {
        Some(UnblockedState::MergeReady)
    } else {
        None
    }
}

fn aggregate_ci(buckets: &[CheckBucket]) -> Option<CiState> {
    if buckets
        .iter()
        .any(|b| matches!(b, CheckBucket::Fail | CheckBucket::Cancel))
    {
        Some(CiState::Fail)
    } else if buckets
        .iter()
        .any(|b| matches!(b, CheckBucket::ActionRequired))
    {
        Some(CiState::ActionRequired)
    } else if buckets.iter().any(|b| matches!(b, CheckBucket::Pending)) {
        Some(CiState::Pending)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contexts::evaluation::infrastructure::gh::schema::CheckBucket;

    #[test]
    fn aggregate_ci_pending_returns_pending() {
        let buckets = vec![CheckBucket::Pending];
        assert_eq!(aggregate_ci(&buckets), Some(CiState::Pending));
    }

    #[test]
    fn aggregate_ci_fail_takes_priority_over_pending() {
        let buckets = vec![CheckBucket::Fail, CheckBucket::Pending];
        assert_eq!(aggregate_ci(&buckets), Some(CiState::Fail));
    }

    #[test]
    fn aggregate_ci_no_pending_returns_none() {
        let buckets = vec![CheckBucket::Other];
        assert_eq!(aggregate_ci(&buckets), None);
    }

    #[test]
    fn translate_unblocked_merge_state_unknown_returns_none() {
        assert_eq!(translate_unblocked(false, "MERGE_STATE_UNKNOWN"), None);
    }

    #[test]
    fn translate_unblocked_unknown_returns_none() {
        assert_eq!(translate_unblocked(false, "UNKNOWN"), None);
    }

    #[test]
    fn translate_unblocked_blocked_returns_none() {
        assert_eq!(translate_unblocked(false, "BLOCKED"), None);
    }
}

/// GitHub Compare API でベースブランチとの差分コミット数を取得する。
///
/// `base_ref` / `head_ref` が空の場合は `Some(0)` を返す（追跡不要）。
/// 失敗した場合は `None` を返す（呼び出し元が `SyncUnknown` として扱う）。
fn fetch_behind_by(base_ref: &str, head_ref: &str, cwd: Option<&Path>) -> Option<u64> {
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

// ── gh コマンド実行・エラー判別 ──────────────────────────────────────────────

enum GhError {
    NotInstalled,
    AuthRequired,
    NoPr,
    RateLimited,
    ApiError(String),
}

impl From<GhError> for RepositoryError {
    fn from(e: GhError) -> Self {
        match e {
            GhError::NotInstalled | GhError::AuthRequired => RepositoryError::Unauthenticated,
            GhError::NoPr => RepositoryError::NotFound,
            GhError::RateLimited => RepositoryError::RateLimited,
            GhError::ApiError(_) => RepositoryError::Unexpected,
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

fn run_gh(args: &[&str], cwd: Option<&Path>) -> Result<Vec<u8>, GhError> {
    let mut cmd = Command::new("gh");
    cmd.args(args);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }

    let mut child = match cmd.spawn() {
        Err(e) if e.kind() == ErrorKind::NotFound => return Err(GhError::NotInstalled),
        Err(e) => return Err(GhError::ApiError(e.to_string())),
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
                return Err(classify_gh_error(exit_code, &stderr_str));
            }
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(GhError::ApiError("gh command timed out".to_string()));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => return Err(GhError::ApiError(e.to_string())),
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
