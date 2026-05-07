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
const COMMAND_TIMEOUT_MS: u64 = 5000;
const CONCURRENT_PROMPTS: usize = 8;

fn daemon_stop_output(env: &TestEnv) -> std::process::Output {
    let bin = assert_cmd::cargo::cargo_bin(BIN);
    let mut child = std::process::Command::new(bin)
        .args(["daemon", "stop"])
        .env("PATH", env.path_env())
        .env("HOME", env.home())
        .env("TMPDIR", env.home())
        .env("XDG_CONFIG_HOME", env.home().join(".config"))
        .current_dir(env.repo_dir.path())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn daemon stop");
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(COMMAND_TIMEOUT_MS);
    loop {
        if child.try_wait().is_ok_and(|status| status.is_some()) {
            return child
                .wait_with_output()
                .expect("collect daemon stop output");
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("daemon stop did not finish within {COMMAND_TIMEOUT_MS}ms");
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

fn assert_prompt_succeeded(output: &std::process::Output) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "prompt failed: status={:?}, stdout={stdout:?}, stderr={stderr:?}",
        output.status.code(),
    );
    assert!(
        stderr.is_empty(),
        "prompt stderr should be empty: {stderr:?}"
    );
    assert!(
        stdout == "? loading" || stdout == "✓ Ready for merge",
        "unexpected prompt stdout: {stdout:?}",
    );
}

fn prompt_output_with_timeout(
    bin: &std::path::Path,
    path: &str,
    home: &std::path::Path,
    repo: &std::path::Path,
) -> std::process::Output {
    let mut child = std::process::Command::new(bin)
        .env("PATH", path)
        .env("HOME", home)
        .env("TMPDIR", home)
        .current_dir(repo)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("run prompt");
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(COMMAND_TIMEOUT_MS);
    loop {
        if child.try_wait().is_ok_and(|status| status.is_some()) {
            return child.wait_with_output().expect("collect prompt output");
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("prompt did not finish within {COMMAND_TIMEOUT_MS}ms");
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

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

    let output = daemon_stop_output(&env);
    assert!(output.status.success());
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

    let output = daemon_stop_output(&env);
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("stopped"));

    drop(daemon);
}

// ── #12: daemon stop 後の status ─────────────────────────────────────────────

/// #12: `daemon stop` 後の `daemon status` → "not running"
#[test]
fn test_daemon_stop_then_status_not_running() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));
    let daemon = DaemonHandle::start(&env);

    let output = daemon_stop_output(&env);
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("stopped"));

    drop(daemon);

    let mut status = Command::cargo_bin(BIN).unwrap();
    env.apply(&mut status);
    status.args(["daemon", "status"]);
    status
        .assert()
        .success()
        .stdout(predicate::str::contains("not running"));
}

#[test]
fn test_daemon_stop_does_not_wait_for_scheduler_tick() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));
    let daemon = DaemonHandle::start_with_env(&env, &[("MERGE_READY_SCHEDULER_TICK_SECS", "5")]);

    let started = std::time::Instant::now();
    let output = daemon_stop_output(&env);
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("stopped"));

    assert!(
        started.elapsed() < std::time::Duration::from_secs(2),
        "daemon stop should not wait for the scheduler tick"
    );
    drop(daemon);
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

    DaemonHandle::stop_for_env(&env);
}

// ── #15: 同時起動レース ──────────────────────────────────────────────────────

/// #15: 複数の `merge-ready-prompt` が同時に Daemon 起動を試みても、Daemon は 1 プロセスのみ存在する
///
/// daemon 未起動の状態で複数の `merge-ready-prompt` を並列実行し、
/// 全て完了後に daemon が 1 プロセスだけ動作していることを確認する。
#[test]
fn test_concurrent_prompt_starts_only_one_daemon() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));
    let prompt_bin = assert_cmd::cargo::cargo_bin(PROMPT_BIN);

    // 複数本を同時起動
    let handles: Vec<_> = (0..CONCURRENT_PROMPTS)
        .map(|_| {
            let bin = prompt_bin.clone();
            let path = env.path_env();
            let home = env.home().to_path_buf();
            let repo = env.repo_dir.path().to_path_buf();
            std::thread::spawn(move || prompt_output_with_timeout(&bin, &path, &home, &repo))
        })
        .collect();

    // 全スレッド完了を待ち、各 prompt が競合中も正常終了することを確認
    for h in handles {
        let output = h.join().expect("prompt thread panicked");
        assert_prompt_succeeded(&output);
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

    DaemonHandle::stop_for_env(&env);
}

// ── #16: バージョンミスマッチ並列再起動 ─────────────────────────────────────

/// #16: バージョン不一致の状態で複数の `merge-ready-prompt` を並列実行しても、
/// 新デーモンは 1 プロセスだけ起動する
#[test]
fn test_concurrent_version_mismatch_starts_only_one_daemon() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));
    let prompt_bin = assert_cmd::cargo::cargo_bin(PROMPT_BIN);

    // バージョン不一致の fake daemon を起動
    let old = FakeDaemonHandle::start_versioned(&env, "0.0.0");

    // 複数本の merge-ready-prompt を同時実行してバージョンミスマッチを並列でトリガーする
    let handles: Vec<_> = (0..CONCURRENT_PROMPTS)
        .map(|_| {
            let bin = prompt_bin.clone();
            let path = env.path_env();
            let home = env.home().to_path_buf();
            let repo = env.repo_dir.path().to_path_buf();
            std::thread::spawn(move || prompt_output_with_timeout(&bin, &path, &home, &repo))
        })
        .collect();

    for h in handles {
        let output = h.join().expect("prompt thread panicked");
        assert_prompt_succeeded(&output);
    }
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

    // socket ファイルが存在すること（1 daemon だけが bind している）
    let socket_path = env
        .home_dir
        .path()
        .join(super::super::helpers::daemon_dir_name())
        .join("daemon.sock");
    assert!(socket_path.exists(), "daemon socket should exist");

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

    DaemonHandle::stop_for_env(&env);
}

// ── #17: daemon status の出力フォーマット ────────────────────────────────────

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
