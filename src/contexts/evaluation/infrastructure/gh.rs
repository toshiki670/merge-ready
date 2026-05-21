mod error;
mod fetch;
mod mapper;
mod schema;

use std::path::{Path, PathBuf};

use error::{GhError, classify_gh_error};
use fetch::fetch_behind_by;
use mapper::{aggregate_ci, translate_review, translate_sync, translate_unblocked};
use schema::{CheckBucket, GhCheckItem, GhPrListItem, GhRepoViewFull, translate_bucket};
use tokio::task::JoinSet;

use crate::contexts::evaluation::domain::error::RepositoryError;
use crate::contexts::evaluation::domain::prompt::pull_request::state::blocked::CiState;
use crate::contexts::evaluation::domain::prompt::pull_request::state::evaluate;
use crate::contexts::evaluation::domain::prompt::{PrId, Prompt, PullRequest, State};
use crate::contexts::evaluation::infrastructure::git::{current_branch, is_git_repo};
use crate::contexts::evaluation::infrastructure::logger::{ErrorCategory, LogRecord, log_record};

async fn run_gh(cwd: &Path, args: &[&str]) -> Result<Vec<u8>, GhError> {
    match crate::shared::process_gh::run_gh(args, Some(cwd)).await {
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

async fn default_branch(cwd: &Path) -> String {
    match run_gh(cwd, &["repo", "view", "--json", "defaultBranchRef"]).await {
        Ok(bytes) => match serde_json::from_slice::<GhRepoViewFull>(&bytes) {
            Ok(v) => v.default_branch_ref.name,
            Err(_) => String::new(),
        },
        Err(_) => String::new(),
    }
}

async fn is_default_branch(cwd: &Path, branch: &str) -> bool {
    if branch.is_empty() {
        return false;
    }
    branch == default_branch(cwd).await
}

async fn fetch_pr_list(cwd: &Path, branch: &str) -> Result<Vec<GhPrListItem>, GhError> {
    let bytes = run_gh(
        cwd,
        &[
            "pr",
            "list",
            "--head",
            branch,
            "--state",
            "all",
            "--json",
            "number,state,isDraft,mergeable,mergeStateStatus,reviewDecision,baseRefName,headRefName",
        ],
    )
    .await?;
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

async fn fetch_ci_state_for(
    cwd: &Path,
    pr_number: u64,
) -> Result<Option<CiState>, RepositoryError> {
    let pr_num_str = pr_number.to_string();
    let bytes = match run_gh(
        cwd,
        &["pr", "checks", &pr_num_str, "--json", "bucket,state"],
    )
    .await
    {
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

async fn evaluate_single_pr(
    cwd: PathBuf,
    pr_view: GhPrListItem,
) -> Result<PullRequest, RepositoryError> {
    let id = PrId::new(pr_view.number);

    // branch_sync と ci を並列取得
    let base = pr_view.base_ref_name.clone();
    let head = pr_view.head_ref_name.clone();
    let mergeable = pr_view.mergeable.clone();
    let pr_number = pr_view.number;
    let cwd_for_sync = cwd.clone();
    let cwd_for_ci = cwd.clone();

    let (branch_sync, ci_result) = tokio::join!(
        async move {
            let behind_by = fetch_behind_by(&base, &head, Some(&cwd_for_sync)).await;
            translate_sync(&mergeable, behind_by)
        },
        async move { fetch_ci_state_for(&cwd_for_ci, pr_number).await },
    );

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
pub async fn fetch_prompt(cwd: &Path) -> Result<Prompt, RepositoryError> {
    if !is_git_repo(cwd) {
        return Ok(Prompt::NoRepository);
    }

    // `git branch --show-current` は 1 回だけ起動し、結果を両ヘルパーで使い回す。
    let branch = current_branch(cwd).await.unwrap_or_default();

    let all_prs = match fetch_pr_list(cwd, &branch).await {
        Ok(list) => list,
        Err(GhError::NoPr) => return Ok(Prompt::NoPullRequest),
        Err(GhError::NotGithubRepository) => return Ok(Prompt::UnsupportedRepository),
        Err(e) => return Err(log_and_convert(e)),
    };

    // PR が一度も作られていない場合
    if all_prs.is_empty() {
        if is_default_branch(cwd, &branch).await {
            return Ok(Prompt::DefaultBranch);
        }
        return Ok(Prompt::NoPullRequest);
    }

    let open_prs: Vec<GhPrListItem> = all_prs.into_iter().filter(|p| p.state == "OPEN").collect();

    // オープン PR がなく全て MERGED/CLOSED → ターミナル状態（空 Vec で表現）
    if open_prs.is_empty() {
        return Ok(Prompt::PullRequests(vec![]));
    }

    // オープン PR のみを evaluate（順序を保つため index を付与）
    let mut set: JoinSet<(usize, Result<PullRequest, RepositoryError>)> = JoinSet::new();
    for (idx, pr_view) in open_prs.into_iter().enumerate() {
        let cwd = cwd.to_path_buf();
        set.spawn(async move {
            let result = evaluate_single_pr(cwd, pr_view).await;
            (idx, result)
        });
    }

    let mut indexed: Vec<(usize, Result<PullRequest, RepositoryError>)> =
        Vec::with_capacity(set.len());
    while let Some(join_result) = set.join_next().await {
        let pair = join_result.expect("pr evaluation task panicked");
        indexed.push(pair);
    }
    indexed.sort_by_key(|(idx, _)| *idx);

    let prs: Result<Vec<PullRequest>, RepositoryError> =
        indexed.into_iter().map(|(_, r)| r).collect();
    Ok(Prompt::PullRequests(prs?))
}
