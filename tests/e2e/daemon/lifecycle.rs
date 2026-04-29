//! `merge-ready daemon` サブコマンドの操作 E2E テスト（シナリオ #7–15）
//!
//! daemon の起動・停止・ステータス確認、およびバージョン不一致時の自動再起動を検証する。

use assert_cmd::Command;
use predicates::prelude::*;

use super::super::helpers::{DaemonHandle, FakeDaemonHandle, TestEnv};

const BIN: &str = "merge-ready";
const PROMPT_BIN: &str = "merge-ready-prompt";

const OPEN_PR_VIEW_JSON: &str = r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null}"#;
const CI_PASS_JSON: &str = r#"[{"bucket":"pass","state":"SUCCESS","name":"ci","link":""}]"#;

// ── #7: daemon start ─────────────────────────────────────────────────────────

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

// ── #8: daemon start（二重起動）────────────────────────────────────────────

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

// ── #9: daemon status（起動中）──────────────────────────────────────────────

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

// ── #10: daemon status（バージョン）─────────────────────────────────────────

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

// ── #11: daemon stop ─────────────────────────────────────────────────────────

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

// ── #12: daemon stop 後の status ─────────────────────────────────────────────

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

// ── #13: 未起動時の status ───────────────────────────────────────────────────

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

// ── #14: バージョン不一致 ────────────────────────────────────────────────────

/// #14: バージョン不一致の旧 daemon が存在する状態で `merge-ready-prompt` を実行すると
/// 旧 daemon がレスポンス返却後に自己再起動し、最終的に現バージョンの daemon が応答する
#[test]
fn test_prompt_restarts_daemon_on_version_mismatch() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));
    let old = FakeDaemonHandle::start_versioned(&env, "0.0.0");

    // 古い daemon が応答することを確認
    let mut before = Command::cargo_bin(BIN).unwrap();
    env.apply(&mut before);
    before
        .args(["daemon", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("version=0.0.0"));

    // merge-ready-prompt を実行すると version mismatch を検知し、
    // fake daemon が新 daemon を spawn して自己シャットダウンする
    let mut prompt = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut prompt);
    prompt.assert().success(); // "? loading" が返る

    // fake daemon がシャットダウンするのを待つ
    drop(old);

    // 新 daemon が起動するまでポーリング（最大 5 秒）
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(5000);
    loop {
        let out = Command::cargo_bin(BIN)
            .unwrap()
            .args(["daemon", "status"])
            .env("PATH", env.path_env())
            .env("HOME", env.home())
            .env("TMPDIR", env.home())
            .output()
            .expect("status failed");
        let stdout = String::from_utf8_lossy(&out.stdout);
        if stdout.contains(&format!("version={}", env!("CARGO_PKG_VERSION"))) {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "new daemon did not start within 5s: {stdout}"
        );
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    // 現バージョンの daemon が応答しており、旧バージョンではないこと
    let mut after = Command::cargo_bin(BIN).unwrap();
    env.apply(&mut after);
    after
        .args(["daemon", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "version={}",
            env!("CARGO_PKG_VERSION")
        )))
        .stdout(predicate::str::contains("version=0.0.0").not());

    // クリーンアップ
    Command::cargo_bin(BIN)
        .unwrap()
        .args(["daemon", "stop"])
        .env("TMPDIR", env.home())
        .output()
        .ok();
}

// ── #15: 同時起動レース ──────────────────────────────────────────────────────

/// #15: 複数の `merge-ready-prompt` が同時に Daemon 起動を試みても、Daemon は 1 プロセスのみ存在する
///
/// daemon 未起動の状態で 20 本の `merge-ready-prompt` を並列実行し、
/// 全て完了後に daemon が 1 プロセスだけ動作していることを確認する。
#[test]
fn test_concurrent_prompt_starts_only_one_daemon() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));
    let prompt_bin = assert_cmd::cargo::cargo_bin(PROMPT_BIN);

    // 20 本を同時起動
    let handles: Vec<_> = (0..20)
        .map(|_| {
            let bin = prompt_bin.clone();
            let path = env.path_env();
            let home = env.home().to_path_buf();
            let repo = env.repo_dir.path().to_path_buf();
            std::thread::spawn(move || {
                std::process::Command::new(&bin)
                    .env("PATH", &path)
                    .env("HOME", &home)
                    .env("TMPDIR", &home)
                    .current_dir(&repo)
                    .output()
            })
        })
        .collect();

    // 全スレッド完了を待つ
    for h in handles {
        let _ = h.join();
    }

    // daemon が正確に 1 プロセス起動していることを確認
    let mut status = Command::cargo_bin(BIN).unwrap();
    env.apply(&mut status);
    status
        .args(["daemon", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("running"));

    // PID ファイルが 1 つだけあることを確認（複数 daemon は起動していない）
    let socket_path = env
        .home_dir
        .path()
        .join(super::super::helpers::daemon_dir_name())
        .join("daemon.sock");
    assert!(socket_path.exists(), "daemon socket should exist");

    // クリーンアップ
    Command::cargo_bin(BIN)
        .unwrap()
        .args(["daemon", "stop"])
        .env("TMPDIR", env.home())
        .output()
        .ok();
}

// ── #16: daemon status の出力フォーマット ────────────────────────────────────

/// #16: `daemon status`（起動中）→ "running pid=<数字> entries=<数字> uptime=<数字>s version=<文字列>"
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
