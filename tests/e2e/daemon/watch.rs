//! `merge-ready watch` コマンドの E2E テスト（シナリオ #W1–#W3）

use std::io::{BufRead, BufReader};
use std::process::Stdio;
use std::sync::mpsc;
use std::time::Duration;

use assert_cmd::Command;
use predicates::prelude::*;

use super::super::helpers::{DaemonHandle, TestEnv};

const BIN: &str = "merge-ready";
const OPEN_PR_VIEW_JSON: &str = r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null}"#;
const CI_PASS_JSON: &str = r#"[{"bucket":"pass","state":"SUCCESS","name":"ci","link":""}]"#;

/// `merge-ready watch` を起動し、`n` 行分の stdout を最大 `timeout` で読んで返す。
/// 読み取り後に SIGINT を送りプロセスを終了させる。
fn spawn_watch_and_read(env: &TestEnv, n: usize, timeout: Duration) -> String {
    let bin = assert_cmd::cargo::cargo_bin(BIN);
    let mut child = std::process::Command::new(bin)
        .args(["watch"])
        .env("PATH", env.path_env())
        .env("HOME", env.home())
        .env("TMPDIR", env.home())
        .env("XDG_CONFIG_HOME", env.home().join(".config"))
        .current_dir(env.repo_dir.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn merge-ready watch");

    let stdout = child.stdout.take().expect("stdout not captured");
    let (tx, rx) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        let mut out = String::new();
        for line in reader.lines().take(n) {
            match line {
                Ok(l) => {
                    out.push_str(&l);
                    out.push('\n');
                }
                Err(_) => break,
            }
        }
        let _ = tx.send(out);
    });

    let output = rx.recv_timeout(timeout).unwrap_or_default();

    let _ = std::process::Command::new("kill")
        .args(["-INT", &child.id().to_string()])
        .status();
    let _ = child.wait();

    output
}

// ── #W1: daemon 未起動 ────────────────────────────────────────────────────────

/// #W1: daemon 未起動時に `watch` → "not running" を出力して exit 非 0
#[test]
fn test_watch_daemon_not_running() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));

    let mut cmd = Command::cargo_bin(BIN).unwrap();
    env.apply(&mut cmd);
    cmd.args(["watch"]);
    cmd.assert()
        .failure()
        .stdout(predicate::str::contains("not running"));
}

// ── #W2: daemon 起動後、ヘッダ表示 ───────────────────────────────────────────

/// #W2: daemon 起動後に `watch` → テーブルヘッダを含む出力（Ctrl+C で終了）
#[test]
fn test_watch_shows_header_when_daemon_running() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));
    let _daemon = DaemonHandle::start(&env);

    let output = spawn_watch_and_read(&env, 1, Duration::from_secs(5));

    assert!(
        output.contains("CWD") || output.contains("no entries cached"),
        "unexpected output: {output:?}"
    );
}

// ── #W3: エントリが存在しない場合 ─────────────────────────────────────────────

/// #W3: daemon 起動直後（エントリなし）は "no entries cached" を表示
#[test]
fn test_watch_shows_no_entries_when_empty() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));
    let _daemon = DaemonHandle::start(&env);

    let output = spawn_watch_and_read(&env, 1, Duration::from_secs(5));

    assert!(
        output.contains("no entries cached"),
        "unexpected output: {output:?}"
    );
}

// ── #W4: PR 番号カラム ────────────────────────────────────────────────────────

/// #W4: cache に PR が存在する場合、watch は PR 番号カラムを表示する
#[test]
fn test_watch_shows_pr_column_for_cached_pr() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));
    let _daemon = DaemonHandle::start(&env);
    DaemonHandle::wait_for_cache(&env, 5000);

    let output = spawn_watch_and_read(&env, 2, Duration::from_secs(5));

    assert!(output.contains("PR"), "unexpected output: {output:?}");
    assert!(output.contains("#1"), "unexpected output: {output:?}");
    assert!(
        output.contains("✓ Ready for merge"),
        "unexpected output: {output:?}"
    );
}

/// #W5: PR が存在しない場合、watch は PR 番号カラムを空欄にする
#[test]
fn test_watch_leaves_pr_column_empty_without_pr() {
    let env = TestEnv::with_no_pr();
    let _daemon = DaemonHandle::start(&env);
    DaemonHandle::wait_for_cache(&env, 5000);

    let output = spawn_watch_and_read(&env, 2, Duration::from_secs(5));

    assert!(output.contains("PR"), "unexpected output: {output:?}");
    assert!(
        output.contains("+ Create PR"),
        "unexpected output: {output:?}"
    );
    assert!(!output.contains('#'), "unexpected output: {output:?}");
}

/// #W6: 複数 PR がある場合、watch は 1 PR 1 行に展開して表示する
#[test]
fn test_watch_expands_multiple_prs_to_rows() {
    let pr_list_json = r#"[
        {"number":200,"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null,"baseRefName":"","headRefName":""},
        {"number":201,"state":"OPEN","isDraft":true,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null,"baseRefName":"","headRefName":""}
    ]"#;
    let env = TestEnv::with_pr_list(pr_list_json, Some(r#"[]"#));
    let _daemon = DaemonHandle::start(&env);
    DaemonHandle::wait_for_cache(&env, 5000);

    let output = spawn_watch_and_read(&env, 3, Duration::from_secs(5));

    assert!(output.contains("PR"), "unexpected output: {output:?}");
    assert!(output.contains("#200"), "unexpected output: {output:?}");
    assert!(output.contains("#201"), "unexpected output: {output:?}");
    assert!(
        output.contains("✓ Ready for merge"),
        "unexpected output: {output:?}"
    );
    assert!(
        output.contains("✎ Ready for review"),
        "unexpected output: {output:?}"
    );
}
