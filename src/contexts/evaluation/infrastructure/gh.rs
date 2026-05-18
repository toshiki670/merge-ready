mod error;
mod fetch;
mod mapper;
mod schema;

use std::path::Path;

use error::{GhError, classify_gh_error};
use fetch::fetch_behind_by;
use mapper::{aggregate_ci, translate_review, translate_sync, translate_unblocked};
use schema::{CheckBucket, GhCheckItem, GhPrListItem, GhRepoViewFull, translate_bucket};

use crate::contexts::evaluation::domain::error::RepositoryError;
use crate::contexts::evaluation::domain::prompt::pull_request::state::blocked::CiState;
use crate::contexts::evaluation::domain::prompt::pull_request::state::evaluate;
use crate::contexts::evaluation::domain::prompt::{PrId, Prompt, PullRequest, State};
use crate::contexts::evaluation::infrastructure::git::{current_branch, is_git_repo};
use crate::contexts::evaluation::infrastructure::logger::{ErrorCategory, LogRecord, log_record};

fn run_gh(cwd: &Path, args: &[&str]) -> Result<Vec<u8>, GhError> {
    match crate::shared::process_gh::run_gh(args, Some(cwd)) {
        Ok(bytes) => Ok(bytes),
        Err(crate::shared::process_gh::GhProcessError::NotInstalled) => Err(GhError::NotInstalled),
        Err(crate::shared::process_gh::GhProcessError::Timeout) => Err(GhError::Timeout),
        Err(crate::shared::process_gh::GhProcessError::Failed { exit_code, stderr }) => {
            Err(classify_gh_error(exit_code, &stderr))
        }
    }
}

fn log_and_convert(e: GhError) -> RepositoryError {
    match &e {
        GhError::AuthRequired => {
            log_record(&LogRecord {
                category: ErrorCategory::Auth,
                detail: None,
            });
        }
        GhError::Timeout => {
            log_record(&LogRecord {
                category: ErrorCategory::Timeout,
                detail: Some("gh command timed out".to_string()),
            });
        }
        GhError::ApiError(msg) => {
            log_record(&LogRecord {
                category: ErrorCategory::Unknown,
                detail: Some(msg.clone()),
            });
        }
        _ => {}
    }
    RepositoryError::from(e)
}

fn default_branch(cwd: &Path) -> String {
    match run_gh(cwd, &["repo", "view", "--json", "defaultBranchRef"]) {
        Ok(bytes) => match serde_json::from_slice::<GhRepoViewFull>(&bytes) {
            Ok(v) => v.default_branch_ref.name,
            Err(_) => String::new(),
        },
        Err(_) => String::new(),
    }
}

fn is_default_branch(cwd: &Path) -> bool {
    let branch = current_branch(cwd).unwrap_or_default();
    if branch.is_empty() {
        return false;
    }
    branch == default_branch(cwd)
}

fn fetch_pr_list(cwd: &Path) -> Result<Vec<GhPrListItem>, GhError> {
    let branch = current_branch(cwd).unwrap_or_default();
    let bytes = run_gh(
        cwd,
        &[
            "pr",
            "list",
            "--head",
            &branch,
            "--state",
            "all",
            "--json",
            "number,state,isDraft,mergeable,mergeStateStatus,reviewDecision,baseRefName,headRefName",
        ],
    )?;
    let mut items: Vec<GhPrListItem> = serde_json::from_slice(&bytes).map_err(|e| {
        log_record(&LogRecord {
            category: ErrorCategory::Unknown,
            detail: Some(e.to_string()),
        });
        GhError::ApiError(e.to_string())
    })?;
    items.sort_by_key(|i| i.number);
    Ok(items)
}

fn fetch_ci_state_for(cwd: &Path, pr_number: u64) -> Result<Option<CiState>, RepositoryError> {
    let pr_num_str = pr_number.to_string();
    let bytes = match run_gh(
        cwd,
        &["pr", "checks", &pr_num_str, "--json", "bucket,state"],
    ) {
        Ok(b) => b,
        Err(GhError::ApiError(msg)) if msg.contains("no checks reported") => {
            return Ok(None);
        }
        Err(e) => return Err(log_and_convert(e)),
    };
    let items: Vec<GhCheckItem> = serde_json::from_slice(&bytes).map_err(|e| {
        log_record(&LogRecord {
            category: ErrorCategory::Unknown,
            detail: Some(e.to_string()),
        });
        RepositoryError::Unexpected
    })?;
    let buckets: Vec<CheckBucket> = items.iter().map(|c| translate_bucket(&c.bucket)).collect();
    Ok(aggregate_ci(&buckets))
}

fn evaluate_single_pr(cwd: &Path, pr_view: &GhPrListItem) -> Result<PullRequest, RepositoryError> {
    let id = PrId::new(pr_view.number);

    // branch_sync と ci を並列取得
    let (branch_sync, ci_result) = std::thread::scope(|s| {
        let base = pr_view.base_ref_name.as_str();
        let head = pr_view.head_ref_name.as_str();
        let mergeable = pr_view.mergeable.as_str();
        let pr_number = pr_view.number;

        let sync_handle = s.spawn(move || {
            let behind_by = fetch_behind_by(base, head, Some(cwd));
            translate_sync(mergeable, behind_by)
        });
        let ci_handle = s.spawn(move || fetch_ci_state_for(cwd, pr_number));

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

/// 指定された作業ディレクトリの PR 状態を取得する。
///
/// gh サブプロセスを起動して情報を集約する **副作用つき** Shell 関数。
/// 失敗は `RepositoryError` として返し、必要なログは `log_record` 経由で
/// このモジュール内で書き出す（ジェネリクスや trait ハンドルは介さない）。
///
/// # Errors
/// gh コマンドの失敗や API レート制限などインフラ起因のエラーを返す。
pub fn fetch_prompt(cwd: &Path) -> Result<Prompt, RepositoryError> {
    if !is_git_repo(cwd) {
        return Ok(Prompt::NoRepository);
    }

    let all_prs = match fetch_pr_list(cwd) {
        Ok(list) => list,
        Err(GhError::NoPr) => return Ok(Prompt::NoPullRequest),
        Err(GhError::NotGithubRepository) => return Ok(Prompt::UnsupportedRepository),
        Err(e) => return Err(log_and_convert(e)),
    };

    // PR が一度も作られていない場合
    if all_prs.is_empty() {
        if is_default_branch(cwd) {
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
            .map(|pr_view| s.spawn(|| evaluate_single_pr(cwd, pr_view)))
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("pr evaluation thread panicked"))
            .collect()
    });

    let prs: Result<Vec<PullRequest>, RepositoryError> = results.into_iter().collect();
    Ok(Prompt::PullRequests(prs?))
}
