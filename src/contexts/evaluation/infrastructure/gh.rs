mod command;
mod error;
mod fetch;
mod mapper;
mod schema;

use command::{CommandRunner, GhProcessRunner};
use error::GhError;
use fetch::fetch_behind_by;
use mapper::{aggregate_ci, translate_review, translate_sync, translate_unblocked};
use schema::{CheckBucket, GhCheckItem, GhPrListItem, GhRepoViewFull, translate_bucket};

use crate::contexts::evaluation::application::port::{ErrorCategory, ErrorLogger, LogRecord};
use crate::contexts::evaluation::domain::error::RepositoryError;
use crate::contexts::evaluation::domain::prompt::pull_request::state::blocked::ci::CiState;
use crate::contexts::evaluation::domain::prompt::pull_request::state::evaluate;
use crate::contexts::evaluation::domain::prompt::{
    PrId, Prompt, PromptRepository, PullRequest, State,
};
use crate::contexts::evaluation::infrastructure::git::{current_branch, is_git_repo};

// ── GhClient ────────────────────────────────────────────────────────────────

pub struct GhClient<L> {
    cwd: Option<std::path::PathBuf>,
    runner: Box<dyn CommandRunner>,
    logger: L,
}

impl<L: ErrorLogger + Sync> GhClient<L> {
    #[must_use]
    pub fn new_in(cwd: std::path::PathBuf, logger: L) -> Self {
        Self {
            runner: Box::new(GhProcessRunner::new(Some(cwd.clone()))),
            cwd: Some(cwd),
            logger,
        }
    }

    fn run_gh(&self, args: &[&str]) -> Result<Vec<u8>, GhError> {
        self.runner.run(args)
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
            let runner: &dyn CommandRunner = &*self.runner;
            let base = pr_view.base_ref_name.as_str();
            let head = pr_view.head_ref_name.as_str();
            let mergeable = pr_view.mergeable.as_str();
            let pr_number = pr_view.number;

            let sync_handle = s.spawn(move || {
                let behind_by = fetch_behind_by(base, head, runner);
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

// ── PromptRepository 実装 ────────────────────────────────────────────────────────

impl<L: ErrorLogger + Sync> PromptRepository for GhClient<L> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contexts::evaluation::application::port::LogRecord;
    use std::sync::{Arc, Mutex};
    use tempfile::TempDir;

    struct StubRunner(fn(&[&str]) -> Result<Vec<u8>, GhError>);
    impl CommandRunner for StubRunner {
        fn run(&self, args: &[&str]) -> Result<Vec<u8>, GhError> {
            (self.0)(args)
        }
    }

    struct NoOpLogger;
    impl ErrorLogger for NoOpLogger {
        fn log(&self, _: &LogRecord) {}
    }

    /// ログ呼び出し回数を数えるスレッドセーフなロガー。Clone で共有する。
    #[derive(Clone)]
    struct CountLogger(Arc<Mutex<u32>>);
    impl CountLogger {
        fn new() -> Self {
            Self(Arc::new(Mutex::new(0)))
        }
        fn count(&self) -> u32 {
            *self.0.lock().unwrap()
        }
    }
    impl ErrorLogger for CountLogger {
        fn log(&self, _: &LogRecord) {
            *self.0.lock().unwrap() += 1;
        }
    }

    fn stub_client(
        runner: impl CommandRunner + 'static,
        cwd: Option<std::path::PathBuf>,
    ) -> GhClient<NoOpLogger> {
        GhClient {
            cwd,
            runner: Box::new(runner),
            logger: NoOpLogger,
        }
    }

    // ── default_branch ────────────────────────────────────────────────────────

    #[test]
    fn default_branch_returns_empty_on_command_failure() {
        let client = stub_client(
            StubRunner(|_| Err(GhError::ApiError("gh error".to_owned()))),
            None,
        );
        assert_eq!(client.default_branch(), "");
    }

    #[test]
    fn default_branch_returns_empty_on_parse_failure() {
        let client = stub_client(StubRunner(|_| Ok(b"not-json".to_vec())), None);
        assert_eq!(client.default_branch(), "");
    }

    // ── is_default_branch ─────────────────────────────────────────────────────

    #[test]
    fn is_default_branch_returns_false_when_current_branch_empty() {
        // 非 git ディレクトリ → current_branch = None → "" → is_empty → false
        let dir = TempDir::new().unwrap();
        let client = stub_client(
            StubRunner(|_| Ok(b"{}".to_vec())),
            Some(dir.path().to_path_buf()),
        );
        assert!(!client.is_default_branch());
    }

    // ── fetch_pr_list ─────────────────────────────────────────────────────────

    #[test]
    fn fetch_pr_list_logs_and_errors_on_parse_failure() {
        let logger = CountLogger::new();
        let client = GhClient {
            cwd: None,
            runner: Box::new(StubRunner(|_| Ok(b"not-json-array".to_vec()))),
            logger: logger.clone(),
        };
        let result = client.fetch_pr_list();
        assert!(matches!(result, Err(GhError::ApiError(_))));
        assert!(
            logger.count() > 0,
            "JSON parse 失敗時にログが記録されるべき"
        );
    }

    // ── fetch_ci_state_for ────────────────────────────────────────────────────

    #[test]
    fn fetch_ci_state_for_maps_non_no_checks_gh_error() {
        let client = stub_client(
            StubRunner(|_| Err(GhError::ApiError("some other error".to_owned()))),
            None,
        );
        let result = client.fetch_ci_state_for(42);
        assert!(matches!(result, Err(RepositoryError::Unexpected)));
    }

    #[test]
    fn fetch_ci_state_for_returns_unexpected_on_parse_failure() {
        let logger = CountLogger::new();
        let client = GhClient {
            cwd: None,
            runner: Box::new(StubRunner(|_| Ok(b"not-json".to_vec()))),
            logger: logger.clone(),
        };
        let result = client.fetch_ci_state_for(42);
        assert!(matches!(result, Err(RepositoryError::Unexpected)));
        assert!(
            logger.count() > 0,
            "JSON parse 失敗時にログが記録されるべき"
        );
    }
}
