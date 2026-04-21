//! daemon ライフサイクルの e2e テスト
//!
//! daemon の起動・停止・ステータス確認を検証する。

use assert_cmd::Command;
use predicates::prelude::*;

use super::super::helpers::{DaemonHandle, TestEnv};

const BIN: &str = "merge-ready";

/// daemon start → "daemon started" を出力して exit 0
#[test]
fn test_daemon_start_prints_started() {
    let env = TestEnv::new(
        r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null}"#,
        Some(r#"[{"bucket":"pass","state":"SUCCESS","name":"ci","link":""}]"#),
    );

    let mut cmd = Command::cargo_bin(BIN).unwrap();
    env.apply(&mut cmd);
    cmd.args(["daemon", "start"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("daemon started"));

    // 後始末
    let mut stop = Command::cargo_bin(BIN).unwrap();
    env.apply(&mut stop);
    stop.args(["daemon", "stop"]);
    stop.assert().success();
}

/// daemon start を二重起動すると "already running" を stderr に出力して exit 非 0
#[test]
fn test_daemon_start_already_running_fails() {
    let env = TestEnv::new(
        r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null}"#,
        Some(r#"[{"bucket":"pass","state":"SUCCESS","name":"ci","link":""}]"#),
    );
    let _daemon = DaemonHandle::start(&env);

    let mut cmd = Command::cargo_bin(BIN).unwrap();
    env.apply(&mut cmd);
    cmd.args(["daemon", "start"]);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("already running"));
}

/// daemon status → "running" を含む出力
#[test]
fn test_daemon_status_shows_running() {
    let env = TestEnv::new(
        r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null}"#,
        Some(r#"[{"bucket":"pass","state":"SUCCESS","name":"ci","link":""}]"#),
    );
    let _daemon = DaemonHandle::start(&env);

    let mut cmd = Command::cargo_bin(BIN).unwrap();
    env.apply(&mut cmd);
    cmd.args(["daemon", "status"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("running"));
}

/// daemon status が version を出力する
#[test]
fn test_daemon_status_includes_version() {
    let env = TestEnv::new(
        r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null}"#,
        Some(r#"[{"bucket":"pass","state":"SUCCESS","name":"ci","link":""}]"#),
    );
    let _daemon = DaemonHandle::start(&env);

    let mut cmd = Command::cargo_bin(BIN).unwrap();
    env.apply(&mut cmd);
    cmd.args(["daemon", "status"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

/// daemon stop 後 → status が "not running"
#[test]
fn test_daemon_stop_terminates() {
    let env = TestEnv::new(
        r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null}"#,
        Some(r#"[{"bucket":"pass","state":"SUCCESS","name":"ci","link":""}]"#),
    );
    let daemon = DaemonHandle::start(&env);

    // 明示的に stop
    let mut stop = Command::cargo_bin(BIN).unwrap();
    env.apply(&mut stop);
    stop.args(["daemon", "stop"]);
    stop.assert()
        .success()
        .stdout(predicate::str::contains("stopped"));

    // DaemonHandle の Drop が二重停止しないよう少し待つ
    std::thread::sleep(std::time::Duration::from_millis(100));
    drop(daemon);

    // status が "not running" を返すこと
    let mut status = Command::cargo_bin(BIN).unwrap();
    env.apply(&mut status);
    status.args(["daemon", "status"]);
    status
        .assert()
        .success()
        .stdout(predicate::str::contains("not running"));
}

/// daemon が未起動のときの status → "not running"
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
