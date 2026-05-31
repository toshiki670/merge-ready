use serde::Deserialize;

/// `gh api graphql` のレスポンス封筒（`{"data":{"repository":...}}`）。
///
/// GraphQL エラー時は gh が非ゼロ終了し stderr 経由で分類されるため、
/// ここでは正常応答（`data.repository`）のみを表現する。`repository` は
/// `NOT_FOUND` 等で `null` になり得るため `Option`。
#[derive(Deserialize)]
pub(super) struct GraphQlResponse {
    pub(super) data: Option<GraphQlData>,
}

#[derive(Deserialize)]
pub(super) struct GraphQlData {
    pub(super) repository: Option<Repository>,
}

#[derive(Deserialize)]
pub(super) struct Repository {
    #[serde(rename = "defaultBranchRef")]
    pub(super) default_branch_ref: Option<DefaultBranchRef>,
    #[serde(rename = "pullRequests")]
    pub(super) pull_requests: PullRequests,
}

#[derive(Deserialize)]
pub(super) struct DefaultBranchRef {
    pub(super) name: String,
}

#[derive(Deserialize)]
pub(super) struct PullRequests {
    pub(super) nodes: Vec<GhPrNode>,
}

/// `pullRequests.nodes[*]`。`gh pr list --json ...` の各項目に CI rollup を加えた形。
#[derive(Deserialize)]
pub(super) struct GhPrNode {
    pub(super) number: u64,
    #[serde(default)]
    pub(super) state: String,
    #[serde(rename = "isDraft")]
    pub(super) is_draft: bool,
    pub(super) mergeable: String,
    #[serde(rename = "mergeStateStatus")]
    pub(super) merge_state_status: String,
    #[serde(rename = "reviewDecision")]
    pub(super) review_decision: Option<String>,
    #[serde(rename = "baseRefName", default)]
    pub(super) base_ref_name: String,
    #[serde(rename = "headRefName", default)]
    pub(super) head_ref_name: String,
    pub(super) commits: Commits,
}

#[derive(Deserialize)]
pub(super) struct Commits {
    pub(super) nodes: Vec<CommitNode>,
}

#[derive(Deserialize)]
pub(super) struct CommitNode {
    pub(super) commit: Commit,
}

#[derive(Deserialize)]
pub(super) struct Commit {
    /// チェック未設定の PR では `null`。
    #[serde(rename = "statusCheckRollup")]
    pub(super) status_check_rollup: Option<StatusCheckRollup>,
}

#[derive(Deserialize)]
pub(super) struct StatusCheckRollup {
    pub(super) contexts: Contexts,
}

#[derive(Deserialize)]
pub(super) struct Contexts {
    pub(super) nodes: Vec<CheckContext>,
}

/// `statusCheckRollup.contexts.nodes[*]`。`__typename` で `CheckRun` /
/// `StatusContext` を判別する。未知の型は `Unknown` に倒して無視する。
///
/// `name` / `started_at`（`StatusContext` は `context` / `created_at`）は
/// 同名 check の重複排除に使う。CI を再実行すると rollup には古い run も残るため、
/// gh `pr checks` と同様に名前ごとに最新 run のみを採用する必要がある。
#[derive(Deserialize)]
#[serde(tag = "__typename")]
pub(super) enum CheckContext {
    CheckRun {
        #[serde(default)]
        name: String,
        status: String,
        conclusion: Option<String>,
        #[serde(rename = "startedAt")]
        started_at: Option<String>,
    },
    StatusContext {
        #[serde(default)]
        context: String,
        state: String,
        #[serde(rename = "createdAt")]
        created_at: Option<String>,
    },
    #[serde(other)]
    Unknown,
}

impl CheckContext {
    /// 重複排除のグルーピングキー（CheckRun.name / StatusContext.context）。
    /// 名前を持たない `Unknown` は `None`。
    pub(super) fn name(&self) -> Option<&str> {
        match self {
            CheckContext::CheckRun { name, .. } => Some(name),
            CheckContext::StatusContext { context, .. } => Some(context),
            CheckContext::Unknown => None,
        }
    }

    /// 同名 run の新しさ比較用タイムスタンプ。ISO8601 文字列は辞書順 = 時系列順。
    /// 欠損時は空文字（= 最古扱い）。
    pub(super) fn recency(&self) -> &str {
        match self {
            CheckContext::CheckRun { started_at, .. } => started_at.as_deref().unwrap_or(""),
            CheckContext::StatusContext { created_at, .. } => created_at.as_deref().unwrap_or(""),
            CheckContext::Unknown => "",
        }
    }
}

/// Compare API（`gh api repos/{owner}/{repo}/compare/...`）のレスポンス。
/// branch sync の `behind_by` 取得のため GraphQL 化後も REST で併用する。
#[derive(Deserialize)]
pub(super) struct GhCompare {
    pub(super) behind_by: u64,
}

/// CI チェックの集計用カテゴリ。`aggregate_ci` が消費する。
#[derive(Debug)]
pub(super) enum CheckBucket {
    Fail,
    Cancel,
    ActionRequired,
    Pending,
    Other,
}

/// `statusCheckRollup` の 1 コンテキストを `CheckBucket` に写像する。
///
/// gh CLI（`gh pr checks --json bucket`）の bucket 判定を再現する。
/// `CheckRun` は `status != COMPLETED` を pending、完了時は `conclusion` で分類。
/// `StatusContext` は `state` で分類する。`Fail` / `Cancel` は `aggregate_ci`
/// で同じ `CiState::Fail` に畳まれるため、両者の弁別は最終結果に影響しない。
pub(super) fn context_to_bucket(ctx: &CheckContext) -> CheckBucket {
    match ctx {
        CheckContext::CheckRun {
            status, conclusion, ..
        } => {
            if status != "COMPLETED" {
                return CheckBucket::Pending;
            }
            match conclusion.as_deref() {
                Some("ACTION_REQUIRED") => CheckBucket::ActionRequired,
                Some("CANCELLED") => CheckBucket::Cancel,
                Some("FAILURE" | "TIMED_OUT" | "STARTUP_FAILURE") => CheckBucket::Fail,
                // SUCCESS / NEUTRAL / SKIPPED と未知の conclusion は非ブロッカー扱い。
                _ => CheckBucket::Other,
            }
        }
        CheckContext::StatusContext { state, .. } => match state.as_str() {
            "PENDING" | "EXPECTED" => CheckBucket::Pending,
            "ERROR" | "FAILURE" => CheckBucket::Fail,
            _ => CheckBucket::Other,
        },
        CheckContext::Unknown => CheckBucket::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::assert_matches;

    fn check_run(status: &str, conclusion: Option<&str>) -> CheckContext {
        CheckContext::CheckRun {
            name: "ci".to_owned(),
            status: status.to_owned(),
            conclusion: conclusion.map(str::to_owned),
            started_at: None,
        }
    }

    fn status_context(state: &str) -> CheckContext {
        CheckContext::StatusContext {
            context: "ci".to_owned(),
            state: state.to_owned(),
            created_at: None,
        }
    }

    #[test]
    fn check_run_in_progress_is_pending() {
        assert_matches!(
            context_to_bucket(&check_run("IN_PROGRESS", None)),
            CheckBucket::Pending
        );
    }

    #[test]
    fn check_run_queued_is_pending() {
        assert_matches!(
            context_to_bucket(&check_run("QUEUED", None)),
            CheckBucket::Pending
        );
    }

    #[test]
    fn check_run_success_is_other() {
        assert_matches!(
            context_to_bucket(&check_run("COMPLETED", Some("SUCCESS"))),
            CheckBucket::Other
        );
    }

    #[test]
    fn check_run_skipped_is_other() {
        assert_matches!(
            context_to_bucket(&check_run("COMPLETED", Some("SKIPPED"))),
            CheckBucket::Other
        );
    }

    #[test]
    fn check_run_failure_is_fail() {
        assert_matches!(
            context_to_bucket(&check_run("COMPLETED", Some("FAILURE"))),
            CheckBucket::Fail
        );
    }

    #[test]
    fn check_run_timed_out_is_fail() {
        assert_matches!(
            context_to_bucket(&check_run("COMPLETED", Some("TIMED_OUT"))),
            CheckBucket::Fail
        );
    }

    #[test]
    fn check_run_cancelled_is_cancel() {
        assert_matches!(
            context_to_bucket(&check_run("COMPLETED", Some("CANCELLED"))),
            CheckBucket::Cancel
        );
    }

    #[test]
    fn check_run_action_required_is_action_required() {
        assert_matches!(
            context_to_bucket(&check_run("COMPLETED", Some("ACTION_REQUIRED"))),
            CheckBucket::ActionRequired
        );
    }

    #[test]
    fn status_context_success_is_other() {
        assert_matches!(
            context_to_bucket(&status_context("SUCCESS")),
            CheckBucket::Other
        );
    }

    #[test]
    fn status_context_pending_is_pending() {
        assert_matches!(
            context_to_bucket(&status_context("PENDING")),
            CheckBucket::Pending
        );
    }

    #[test]
    fn status_context_error_is_fail() {
        assert_matches!(
            context_to_bucket(&status_context("ERROR")),
            CheckBucket::Fail
        );
    }

    #[test]
    fn unknown_typename_is_other() {
        assert_matches!(
            context_to_bucket(&CheckContext::Unknown),
            CheckBucket::Other
        );
    }
}
