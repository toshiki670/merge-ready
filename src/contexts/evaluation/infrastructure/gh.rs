mod error;
mod fetch;
mod mapper;
mod schema;

use std::path::{Path, PathBuf};

use error::{GhError, classify_gh_error};
use fetch::fetch_behind_by;
use mapper::{aggregate_ci, translate_review, translate_sync, translate_unblocked};
use schema::{CheckBucket, GhPrNode, GraphQlResponse, Repository, context_to_bucket};
use tokio::task::JoinSet;

use crate::contexts::evaluation::domain::error::RepositoryError;
use crate::contexts::evaluation::domain::prompt::pull_request::state::blocked::CiState;
use crate::contexts::evaluation::domain::prompt::pull_request::state::evaluate;
use crate::contexts::evaluation::domain::prompt::{PrId, Prompt, PullRequest, State};
use crate::contexts::evaluation::infrastructure::git::{current_branch, is_git_repo};
use crate::contexts::evaluation::infrastructure::logger::{ErrorCategory, LogRecord, log_record};

/// PR メタ + CI（statusCheckRollup）+ defaultBranchRef を単一リクエストで取得する
/// GraphQL クエリ。`{owner}` / `{repo}` placeholder は `-F` 経由で gh が現在の
/// リポジトリから補完する。
const REPO_QUERY: &str = "\
query($owner:String!, $repo:String!, $head:String!) {
  repository(owner:$owner, name:$repo) {
    defaultBranchRef { name }
    pullRequests(headRefName:$head, first:100, states:[OPEN,CLOSED,MERGED]) {
      nodes {
        number state isDraft mergeable mergeStateStatus reviewDecision
        baseRefName headRefName
        commits(last:1) { nodes { commit { statusCheckRollup {
          contexts(first:100) { nodes {
            __typename
            ... on CheckRun { status conclusion }
            ... on StatusContext { state }
          } }
        } } } }
      }
    }
  }
}";

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

/// 単一 GraphQL クエリで repository（PR メタ + CI rollup + defaultBranchRef）を取得する。
///
/// `repository` が `null`（解決不能）の場合は `None` を返す。gh が非ゼロ終了した場合は
/// `classify_gh_error`（`run_gh` 内）で分類済みの `GhError` を返す。
async fn fetch_repository(cwd: &Path, branch: &str) -> Result<Option<Repository>, GhError> {
    let head_arg = format!("head={branch}");
    let query_arg = format!("query={REPO_QUERY}");
    let bytes = run_gh(
        cwd,
        &[
            "api",
            "graphql",
            "-F",
            "owner={owner}",
            "-F",
            "repo={repo}",
            "-f",
            head_arg.as_str(),
            "-f",
            query_arg.as_str(),
        ],
    )
    .await?;
    let resp: GraphQlResponse = serde_json::from_slice(&bytes).map_err(|e| {
        log_record(&LogRecord {
            category: ErrorCategory::Unknown,
            detail: Some(e.to_string()),
        });
        GhError::ApiError(e.to_string())
    })?;
    Ok(resp.data.and_then(|d| d.repository))
}

/// PR ノードの `statusCheckRollup` から CI 状態を集約する（追加の gh 起動なし）。
/// チェック未設定（rollup が `null`）の場合は `None`。
fn ci_from_node(node: &GhPrNode) -> Option<CiState> {
    let contexts = node
        .commits
        .nodes
        .first()
        .and_then(|c| c.commit.status_check_rollup.as_ref())
        .map_or(&[][..], |r| r.contexts.nodes.as_slice());
    let buckets: Vec<CheckBucket> = contexts.iter().map(context_to_bucket).collect();
    aggregate_ci(&buckets)
}

async fn evaluate_single_pr(
    cwd: PathBuf,
    pr_view: GhPrNode,
) -> Result<PullRequest, RepositoryError> {
    let id = PrId::new(pr_view.number);

    // CI は GraphQL レスポンス（rollup）から同期的に算出する。
    let ci = ci_from_node(&pr_view);

    // branch sync の behind_by だけは GraphQL に対応フィールドが無いため
    // PR ごとに REST compare を併用する（refresh あたり 1 GraphQL + N compare）。
    let behind_by =
        fetch_behind_by(&pr_view.base_ref_name, &pr_view.head_ref_name, Some(&cwd)).await;
    let branch_sync = translate_sync(&pr_view.mergeable, behind_by);

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

    // `git branch --show-current` は 1 回だけ起動し、結果を使い回す。
    let branch = current_branch(cwd).await.unwrap_or_default();

    // PR メタ + CI + defaultBranchRef を単一 GraphQL で取得する。
    let repository = match fetch_repository(cwd, &branch).await {
        Ok(Some(repo)) => repo,
        // repository が null（解決不能）/ 非 GitHub リモート → 未対応リポジトリ扱い。
        Ok(None) | Err(GhError::NotGithubRepository) => {
            return Ok(Prompt::UnsupportedRepository);
        }
        // GraphQL では通常発生しない（空 nodes で成功する）が、gh が
        // "no pull requests found" 系の stderr を返した場合の保険。
        Err(GhError::NoPr) => return Ok(Prompt::NoPullRequest),
        Err(e) => return Err(log_and_convert(e)),
    };

    let all_prs = repository.pull_requests.nodes;

    // PR が一度も作られていない場合: defaultBranchRef で判定（追加の gh 起動なし）。
    if all_prs.is_empty() {
        let default_branch = repository
            .default_branch_ref
            .map(|d| d.name)
            .unwrap_or_default();
        if !branch.is_empty() && branch == default_branch {
            return Ok(Prompt::DefaultBranch);
        }
        return Ok(Prompt::NoPullRequest);
    }

    let mut open_prs: Vec<GhPrNode> = all_prs.into_iter().filter(|p| p.state == "OPEN").collect();
    // 表示順を安定させるため PR 番号昇順にそろえる。
    open_prs.sort_by_key(|i| i.number);

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
