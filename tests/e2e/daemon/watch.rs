//! `merge-ready watch` コマンドの E2E テスト（シナリオ #W1–#W3）

use assert_cmd::Command;
use predicates::prelude::*;

use super::super::helpers::{DaemonHandle, TestEnv};

const BIN: &str = "merge-ready";
const OPEN_PR_VIEW_JSON: &str = r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null}"#;
const CI_PASS_JSON: &str = r#"[{"bucket":"pass","state":"SUCCESS","name":"ci","link":""}]"#;

// ── #W1: daemon 未起動 ────────────────────────────────────────────────────────

/// #W1: daemon 未起動時に `watch --once` → "not running" を出力して exit 非 0
#[test]
fn test_watch_once_daemon_not_running() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));

    let mut cmd = Command::cargo_bin(BIN).unwrap();
    env.apply(&mut cmd);
    cmd.args(["watch", "--once"]);
    cmd.assert()
        .failure()
        .stdout(predicate::str::contains("not running"));
}

// ── #W2: daemon 起動後、ヘッダ表示 ───────────────────────────────────────────

/// #W2: daemon 起動後に `watch --once` → テーブルヘッダを含む出力
#[test]
fn test_watch_once_shows_header_when_daemon_running() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));
    let _daemon = DaemonHandle::start(&env);

    let mut cmd = Command::cargo_bin(BIN).unwrap();
    env.apply(&mut cmd);
    cmd.args(["watch", "--once"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("CWD").or(predicate::str::contains("no entries cached")));
}

// ── #W3: エントリが存在しない場合 ─────────────────────────────────────────────

/// #W3: daemon 起動直後（エントリなし）は "no entries cached" を表示
#[test]
fn test_watch_once_shows_no_entries_when_empty() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));
    let _daemon = DaemonHandle::start(&env);

    let mut cmd = Command::cargo_bin(BIN).unwrap();
    env.apply(&mut cmd);
    cmd.args(["watch", "--once"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("no entries cached"));
}
