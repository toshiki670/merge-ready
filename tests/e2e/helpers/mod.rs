mod coverage;
mod daemon_handle;
mod env;
mod multi_repo;

pub(crate) use coverage::{apply_coverage_env, apply_coverage_env_assert};
pub use daemon_handle::DaemonHandle;
pub use env::TestEnv;
pub(crate) use env::{setup_empty_dirs, setup_git_dirs};
pub use multi_repo::MultiRepoEnv;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

pub(super) const PROMPT_TIMEOUT_MS: u64 = 5000;

/// `merge-ready-prompt` をタイムアウト付きで実行する。
///
/// `cmd` には `stdin`/`stdout`/`stderr` を設定せずに渡すこと（本関数内で設定する）。
/// `PROMPT_TIMEOUT_MS` 以内に完了しない場合はプロセスを kill してパニックする。
pub(super) fn run_prompt_with_timeout(cmd: &mut std::process::Command) -> std::process::Output {
    let mut child = cmd
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn merge-ready-prompt");
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(PROMPT_TIMEOUT_MS);
    loop {
        if child.try_wait().is_ok_and(|s| s.is_some()) {
            return child.wait_with_output().expect("collect prompt output");
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("merge-ready-prompt did not finish within {PROMPT_TIMEOUT_MS}ms");
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

pub(crate) fn write_executable(path: impl AsRef<Path>, content: &str) {
    let path = path.as_ref();
    fs::write(path, content).expect("failed to write script");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("failed to chmod script");
}

// ── GraphQL レスポンス組み立て（#360） ────────────────────────────────────────
//
// `fetch_prompt` は `gh api graphql` 単一クエリで PR メタ + CI rollup +
// defaultBranchRef を取得する。fake gh はこの封筒形状の JSON を返す。
// branch sync の `behind_by` だけは REST `gh api ...compare...` 併用のまま。

use serde_json::{Value, json};

/// CLEAN + 成功 CI を表す `statusCheckRollup.contexts.nodes`。多くの fixture が利用。
pub(crate) const ROLLUP_PASS: &str =
    r#"[{"__typename":"CheckRun","status":"COMPLETED","conclusion":"SUCCESS"}]"#;

fn commits_value(rollup_contexts: Option<&str>) -> Value {
    let rollup = match rollup_contexts {
        Some(ctx) => {
            let nodes: Value = serde_json::from_str(ctx).expect("invalid rollup contexts json");
            json!({ "contexts": { "nodes": nodes } })
        }
        None => Value::Null,
    };
    json!({ "nodes": [ { "commit": { "statusCheckRollup": rollup } } ] })
}

/// PR フラグメント（`{...}` オブジェクト）に `number` と CI `statusCheckRollup` を付与した
/// PR ノード `Value` を作る。`rollup_contexts` が `None` のとき rollup は `null`（CI 未設定）。
pub(crate) fn pr_node(number: u64, fragment: &str, rollup_contexts: Option<&str>) -> Value {
    let mut obj: Value = serde_json::from_str(fragment).expect("invalid pr fragment json");
    let map = obj.as_object_mut().expect("pr fragment must be an object");
    map.insert("number".to_owned(), json!(number));
    map.insert("commits".to_owned(), commits_value(rollup_contexts));
    obj
}

/// すでに `number` を含む PR ノードに CI `statusCheckRollup` を付与する（複数 PR 用）。
pub(crate) fn attach_rollup(mut node: Value, rollup_contexts: Option<&str>) -> Value {
    let map = node.as_object_mut().expect("pr node must be an object");
    map.insert("commits".to_owned(), commits_value(rollup_contexts));
    node
}

/// `gh api graphql` の repository レスポンス JSON 文字列。
pub(crate) fn graphql_response(nodes: &[Value], default_branch: &str) -> String {
    json!({
        "data": { "repository": {
            "defaultBranchRef": { "name": default_branch },
            "pullRequests": { "nodes": nodes }
        }}
    })
    .to_string()
}

/// 単一 PR の graphql 応答文字列（defaultBranch="main"）。
pub(crate) fn graphql_single(fragment: &str, rollup_contexts: Option<&str>) -> String {
    graphql_response(&[pr_node(1, fragment, rollup_contexts)], "main")
}

/// fake `gh` バイナリ用に `api rate_limit` を「クォータ十分」の静的 JSON で返す
/// シェルスクリプト断片。各 fixture の `case "$*"` よりも前に挿入することで、
/// daemon の `rate_limit` fetcher による予期しないコール記録を防ぐ。
///
/// reset を遠未来（2286 年）に設定しているため、スナップショットは常に「枯渇しない／
/// 残量比率ほぼ 1.0」と扱われ、既存テストの間隔判定に影響しない。
pub(crate) const FAKE_GH_RATE_LIMIT_OK_SNIPPET: &str = r#"case "$*" in
  *'api rate_limit'*)
    printf '%s' '{"resources":{"core":{"limit":5000,"remaining":4999,"reset":9999999999},"graphql":{"limit":5000,"remaining":4999,"reset":9999999999}}}'
    exit 0
    ;;
esac
"#;
