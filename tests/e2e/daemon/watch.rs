//! `merge-ready watch` コマンドの E2E テスト（シナリオ #W1–#W7）

use std::io::{BufRead, BufReader};
use std::process::Stdio;
use std::sync::mpsc;
use std::time::Duration;

use assert_cmd::Command;
use predicates::prelude::*;

use super::super::helpers::{DaemonHandle, MultiRepoEnv, TestEnv, apply_coverage_env};

const BIN: &str = "merge-ready";
const OPEN_PR_VIEW_JSON: &str = r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null}"#;
const CI_PASS_JSON: &str =
    r#"[{"__typename":"CheckRun","status":"COMPLETED","conclusion":"SUCCESS"}]"#;

/// `merge-ready watch` を起動し、`n` 行分の stdout を最大 `timeout` で読んで返す。
/// 読み取り後に SIGINT を送りプロセスを終了させる。
fn spawn_watch_and_read(env: &TestEnv, n: usize, timeout: Duration) -> String {
    let bin = assert_cmd::cargo::cargo_bin(BIN);
    let mut cmd = std::process::Command::new(bin);
    cmd.args(["watch"])
        .env("PATH", env.path_env())
        .env("HOME", env.home())
        .env("TMPDIR", env.home())
        .env("MERGE_READY_BASE_DIR", env.home())
        .env("XDG_CONFIG_HOME", env.home().join(".config"))
        .current_dir(env.repo.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    apply_coverage_env(&mut cmd);
    let mut child = cmd.spawn().expect("failed to spawn merge-ready watch");

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

// ── #W7: 複数リポジトリのソート順 ────────────────────────────────────────────

fn spawn_watch_and_read_multi(env: &MultiRepoEnv, n: usize, timeout: Duration) -> String {
    let bin = assert_cmd::cargo::cargo_bin(BIN);
    let mut cmd = std::process::Command::new(bin);
    cmd.args(["watch"])
        .env("PATH", env.path_env())
        .env("HOME", env.home())
        .env("TMPDIR", env.home())
        .env("MERGE_READY_BASE_DIR", env.home())
        .env("XDG_CONFIG_HOME", env.home().join(".config"))
        .current_dir(env.repo_a.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    apply_coverage_env(&mut cmd);
    let mut child = cmd.spawn().expect("failed to spawn merge-ready watch");

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

/// 画面クリア + カーソル移動の CSI。watch は描画ごとに先頭に挿入する。
const SCREEN_CLEAR: &str = "\x1b[2J\x1b[1;1H";

/// SGR カラーコード（`...m` 終端）を除いた可視文字数を返す。
fn visible_len(s: &str) -> usize {
    let mut len = 0;
    let mut in_esc = false;
    for c in s.chars() {
        if c == '\x1b' {
            in_esc = true;
        } else if in_esc {
            if c == 'm' {
                in_esc = false;
            }
        } else {
            len += 1;
        }
    }
    len
}

/// CACHED AT 列はヘッダーも値も右揃えで描画される（末尾位置で揃う）
#[test]
fn test_watch_right_aligns_cached_at_column() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));
    let _daemon = DaemonHandle::start(&env);
    DaemonHandle::wait_for_cache(&env, 5000);

    let output = spawn_watch_and_read(&env, 2, Duration::from_secs(5));
    let lines: Vec<&str> = output.lines().collect();
    assert!(lines.len() >= 2, "header + data 行が読めるべき: {output:?}");

    let header = lines[0].strip_prefix(SCREEN_CLEAR).unwrap_or(lines[0]);
    let data = lines[1];

    assert!(header.ends_with("CACHED AT"), "header tail: {header:?}");
    assert!(data.ends_with("ago"), "data tail: {data:?}");

    assert_eq!(
        visible_len(header),
        visible_len(data),
        "ヘッダー行とデータ行の表示幅が一致すべき (右端揃え): header={header:?}, data={data:?}",
    );
}

/// #W7: 複数リポジトリがある場合、watch は CWD 昇順でソートして表示する
#[test]
fn test_watch_entries_sorted_by_cwd() {
    let env = MultiRepoEnv::new(OPEN_PR_VIEW_JSON, OPEN_PR_VIEW_JSON);
    let _daemon = env.start_daemon();
    env.wait_for_cache_in(&env.repo_a, 5000);
    env.wait_for_cache_in(&env.repo_b, 5000);

    let output = spawn_watch_and_read_multi(&env, 3, Duration::from_secs(5));

    let cwd_a = env.repo_a.path().to_string_lossy().into_owned();
    let cwd_b = env.repo_b.path().to_string_lossy().into_owned();

    let data_lines: Vec<&str> = output.lines().skip(1).collect();
    let pos_a = data_lines.iter().position(|l| l.contains(&cwd_a));
    let pos_b = data_lines.iter().position(|l| l.contains(&cwd_b));

    assert!(pos_a.is_some(), "repo_a が出力に含まれない: {output:?}");
    assert!(pos_b.is_some(), "repo_b が出力に含まれない: {output:?}");

    if cwd_a < cwd_b {
        assert!(
            pos_a.unwrap() < pos_b.unwrap(),
            "repo_a が repo_b より前に表示されるべき: {output:?}"
        );
    } else {
        assert!(
            pos_b.unwrap() < pos_a.unwrap(),
            "repo_b が repo_a より前に表示されるべき: {output:?}"
        );
    }
}

/// #W6: 複数 PR がある場合、watch は 1 PR 1 行に展開して表示する
#[test]
fn test_watch_expands_multiple_prs_to_rows() {
    let pr_list_json = r#"[
        {"number":200,"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null,"baseRefName":"","headRefName":""},
        {"number":201,"state":"OPEN","isDraft":true,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null,"baseRefName":"","headRefName":""}
    ]"#;
    let env = TestEnv::with_pr_list(pr_list_json, Some(r"[]"));
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
