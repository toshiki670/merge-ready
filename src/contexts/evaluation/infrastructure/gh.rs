mod schema;

use std::io::{ErrorKind, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use schema::{
    CheckBucket, GhCheckItem, GhCompare, GhPrListItem, GhRepoView, GhRepoViewFull, translate_bucket,
};

use crate::contexts::evaluation::application::port::{ErrorCategory, ErrorLogger, LogRecord};
use crate::contexts::evaluation::domain::error::RepositoryError;
use crate::contexts::evaluation::domain::prompt::pull_request::state::blocked::branch_sync::BranchSyncState;
use crate::contexts::evaluation::domain::prompt::pull_request::state::blocked::ci::CiState;
use crate::contexts::evaluation::domain::prompt::pull_request::state::blocked::review::ReviewState;
use crate::contexts::evaluation::domain::prompt::pull_request::state::evaluate;
use crate::contexts::evaluation::domain::prompt::pull_request::state::unblocked::UnblockedState;
use crate::contexts::evaluation::domain::prompt::{PrId, PrRepository, Prompt, PullRequest, State};
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
        match &e {
            GhError::AuthRequired => {
                self.logger.log(&LogRecord {
                    category: ErrorCategory::Auth,
                    detail: None,
                });
            }
            GhError::Timeout => {
                self.logger.log(&LogRecord {
                    category: ErrorCategory::Timeout,
                    detail: Some("gh command timed out".to_string()),
                });
            }
            GhError::ApiError(msg) => {
                self.logger.log(&LogRecord {
                    category: ErrorCategory::Unknown,
                    detail: Some(msg.clone()),
                });
            }
            _ => {}
        }
        RepositoryError::from(e)
    }

    fn default_branch(&self) -> String {
        match self.run_gh(&["repo", "view", "--json", "defaultBranchRef"]) {
            Ok(bytes) => match serde_json::from_slice::<GhRepoViewFull>(&bytes) {
                Ok(v) => v.default_branch_ref.name,
                Err(_) => String::new(),
            },
            Err(_) => String::new(),
        }
    }

    fn is_default_branch(&self) -> bool {
        let branch = current_branch(self.cwd.as_deref()).unwrap_or_default();
        if branch.is_empty() {
            return false;
        }
        branch == self.default_branch()
    }

    fn fetch_pr_list(&self) -> Result<Vec<GhPrListItem>, GhError> {
        let branch = current_branch(self.cwd.as_deref()).unwrap_or_default();
        let bytes = self.run_gh(&[
            "pr",
            "list",
            "--head",
            &branch,
            "--state",
            "all",
            "--json",
            "number,state,isDraft,mergeable,mergeStateStatus,reviewDecision,baseRefName,headRefName",
        ])?;
        let mut items: Vec<GhPrListItem> = serde_json::from_slice(&bytes).map_err(|e| {
            self.logger.log(&LogRecord {
                category: ErrorCategory::Unknown,
                detail: Some(e.to_string()),
            });
            GhError::ApiError(e.to_string())
        })?;
        items.sort_by_key(|i| i.number);
        Ok(items)
    }

    fn fetch_ci_state_for(&self, pr_number: u64) -> Result<Option<CiState>, RepositoryError> {
        let pr_num_str = pr_number.to_string();
        let bytes = match self.run_gh(&["pr", "checks", &pr_num_str, "--json", "bucket,state"]) {
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

    fn evaluate_single_pr(&self, pr_view: &GhPrListItem) -> Result<PullRequest, RepositoryError> {
        let id = PrId::new(pr_view.number);

        // branch_sync と ci を並列取得
        let (branch_sync, ci_result) = std::thread::scope(|s| {
            let cwd = self.cwd.as_deref();
            let base = pr_view.base_ref_name.as_str();
            let head = pr_view.head_ref_name.as_str();
            let mergeable = pr_view.mergeable.as_str();
            let pr_number = pr_view.number;

            let sync_handle = s.spawn(move || {
                let behind_by = fetch_behind_by(base, head, cwd);
                translate_sync(mergeable, behind_by)
            });
            let ci_handle = s.spawn(move || self.fetch_ci_state_for(pr_number));

            (
                sync_handle.join().expect("sync thread panicked"),
                ci_handle.join().expect("ci thread panicked"),
            )
        });

        let ci = ci_result?;

        // GitHub がまだマージ可能性を計算中（他のシグナルより優先）
        if matches!(
            pr_view.merge_state_status.as_str(),
            "MERGE_STATE_UNKNOWN" | "UNKNOWN"
        ) {
            return Ok(PullRequest {
                id,
                state: State::Calculating,
            });
        }

        let review = translate_review(pr_view.review_decision.as_deref());
        let unblocked = translate_unblocked(pr_view.is_draft, &pr_view.merge_state_status);
        let state = evaluate(branch_sync, ci, review, unblocked);

        Ok(PullRequest { id, state })
    }
}

// ── PrRepository 実装 ────────────────────────────────────────────────────────

impl<L: ErrorLogger + Sync> PrRepository for GhClient<L> {
    fn fetch(&self) -> Result<Prompt, RepositoryError> {
        if !is_git_repo(self.cwd.as_deref()) {
            return Ok(Prompt::NoRepository);
        }

        let all_prs = match self.fetch_pr_list() {
            Ok(list) => list,
            Err(GhError::NoPr) => return Ok(Prompt::NoPullRequest),
            Err(GhError::NotGithubRepository) => return Ok(Prompt::UnsupportedRepository),
            Err(e) => return Err(self.log_and_convert(e)),
        };

        // PR が一度も作られていない場合
        if all_prs.is_empty() {
            if self.is_default_branch() {
                return Ok(Prompt::DefaultBranch);
            }
            return Ok(Prompt::NoPullRequest);
        }

        let open_prs: Vec<&GhPrListItem> = all_prs.iter().filter(|p| p.state == "OPEN").collect();

        // オープン PR がなく全て MERGED/CLOSED → ターミナル状態（空 Vec で表現）
        if open_prs.is_empty() {
            return Ok(Prompt::PullRequests(vec![]));
        }

        // オープン PR のみを evaluate
        let results: Vec<Result<PullRequest, RepositoryError>> = std::thread::scope(|s| {
            let handles: Vec<_> = open_prs
                .iter()
                .map(|pr_view| s.spawn(|| self.evaluate_single_pr(pr_view)))
                .collect();
            handles
                .into_iter()
                .map(|h| h.join().expect("pr evaluation thread panicked"))
                .collect()
        });

        let prs: Result<Vec<PullRequest>, RepositoryError> = results.into_iter().collect();
        Ok(Prompt::PullRequests(prs?))
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
    Timeout,
    NotGithubRepository,
    ApiError(String),
}

impl From<GhError> for RepositoryError {
    fn from(e: GhError) -> Self {
        match e {
            GhError::NotInstalled | GhError::AuthRequired => RepositoryError::Unauthenticated,
            GhError::RateLimited => RepositoryError::RateLimited,
            GhError::NoPr
            | GhError::NotGithubRepository
            | GhError::Timeout
            | GhError::ApiError(_) => RepositoryError::Unexpected,
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
                return Err(GhError::Timeout);
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
    } else if exit_code == 1
        && (stderr.contains("no git remotes found")
            || stderr.contains(
                "none of the git remotes configured for this repository point to a known GitHub host",
            ))
    {
        GhError::NotGithubRepository
    } else {
        GhError::ApiError(stderr.to_owned())
    }
}
