//! `merge-ready daemon` サブコマンドの基本 lifecycle E2E テスト。

use assert_cmd::Command;
use predicates::prelude::*;

use super::super::helpers::{DaemonHandle, TestEnv};

const BIN: &str = "merge-ready";
const OPEN_PR_VIEW_JSON: &str = r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null}"#;
const CI_PASS_JSON: &str = r#"[{"bucket":"pass","state":"SUCCESS","name":"ci","link":""}]"#;

/// #7: `daemon start` → "daemon started" を出力して exit 0
#[test]
fn test_daemon_start_prints_started() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));

    let mut cmd = Command::cargo_bin(BIN).unwrap();
    env.apply(&mut cmd);
    cmd.args(["daemon", "start"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("daemon started"));

    let mut stop = Command::cargo_bin(BIN).unwrap();
    env.apply(&mut stop);
    stop.args(["daemon", "stop"]);
    stop.assert().success();
}

/// #8: daemon 起動済みで `daemon start` → "already running" を stderr に出力して exit 非 0
#[test]
fn test_daemon_start_already_running_fails() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));
    let _daemon = DaemonHandle::start(&env);

    let mut cmd = Command::cargo_bin(BIN).unwrap();
    env.apply(&mut cmd);
    cmd.args(["daemon", "start"]);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("already running"));
}

/// #9: `daemon status`（起動中）→ "running" を含む出力
#[test]
fn test_daemon_status_shows_running() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));
    let _daemon = DaemonHandle::start(&env);

    let mut cmd = Command::cargo_bin(BIN).unwrap();
    env.apply(&mut cmd);
    cmd.args(["daemon", "status"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("running"));
}

/// #10: `daemon status`（起動中）→ バージョン文字列を含む
#[test]
fn test_daemon_status_includes_version() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));
    let _daemon = DaemonHandle::start(&env);

    let mut cmd = Command::cargo_bin(BIN).unwrap();
    env.apply(&mut cmd);
    cmd.args(["daemon", "status"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

/// #11: `daemon stop` → "stopped" を出力して exit 0
#[test]
fn test_daemon_stop_prints_stopped() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));
    let daemon = DaemonHandle::start(&env);

    let mut stop = Command::cargo_bin(BIN).unwrap();
    env.apply(&mut stop);
    stop.args(["daemon", "stop"]);
    stop.assert()
        .success()
        .stdout(predicate::str::contains("stopped"));

    std::thread::sleep(std::time::Duration::from_millis(100));
    drop(daemon);
}

/// #12: `daemon stop` 後の `daemon status` → "not running"
#[test]
fn test_daemon_stop_then_status_not_running() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));
    let daemon = DaemonHandle::start(&env);

    let mut stop = Command::cargo_bin(BIN).unwrap();
    env.apply(&mut stop);
    stop.args(["daemon", "stop"]);
    stop.assert()
        .success()
        .stdout(predicate::str::contains("stopped"));

    std::thread::sleep(std::time::Duration::from_millis(100));
    drop(daemon);

    let mut status = Command::cargo_bin(BIN).unwrap();
    env.apply(&mut status);
    status.args(["daemon", "status"]);
    status
        .assert()
        .success()
        .stdout(predicate::str::contains("not running"));
}

/// #13: 未起動時の `daemon status` → "not running"
#[test]
fn test_daemon_status_not_running() {
    let env = TestEnv::new(r#"{"state":"OPEN"}"#, None);

    let mut cmd = Command::cargo_bin(BIN).unwrap();
    env.apply(&mut cmd);
    cmd.args(["daemon", "status"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::diff("not running\n"));
}

/// #17: `daemon status`（起動中）→ "running pid=<数字> entries=<数字> uptime=<数字>s version=<文字列>"
#[test]
fn test_daemon_status_format() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));
    let _daemon = DaemonHandle::start(&env);

    let mut cmd = Command::cargo_bin(BIN).unwrap();
    env.apply(&mut cmd);
    cmd.args(["daemon", "status"]);
    cmd.assert().success().stdout(
        predicate::str::is_match(r"^running  pid=\d+  entries=\d+  uptime=\d+s  version=.+\n$")
            .unwrap(),
    );
}
