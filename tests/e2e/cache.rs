//! キャッシュ機構の e2e テスト
//!
//! daemon 経由のキャッシュ動作を検証する。
//! 各テストは独立した `home_dir`（= `TMPDIR`）を持つため、daemon socket が分離される。

use assert_cmd::Command;
use predicates::prelude::*;

use super::helpers::{DaemonHandle, MultiRepoEnv, TestEnv};

/// merge-ready のバイナリ名
const BIN: &str = "merge-ready";

/// マージ可能な PR の `gh pr view` JSON
const OPEN_PR_VIEW_JSON: &str = r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null}"#;

/// CI が Pass の `gh pr checks` JSON
const CI_PASS_JSON: &str = r#"[{"bucket":"pass","state":"SUCCESS","name":"ci","link":""}]"#;

// ── daemon なし（初回起動） ─────────────────────────────────────────────

/// daemon 未起動時 → `? loading` を出力してバックグラウンドで daemon を起動する
#[test]
fn test_cache_miss_shows_loading() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));

    let mut cmd = Command::cargo_bin(BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert()
        .success()
        .stdout(predicate::str::diff("? loading"));
}

// ── daemon あり ─────────────────────────────────────────────────────────

/// daemon 起動直後（キャッシュなし）→ `? loading`
#[test]
fn test_daemon_miss_shows_loading() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));
    let _daemon = DaemonHandle::start(&env);

    let mut cmd = Command::cargo_bin(BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert()
        .success()
        .stdout(predicate::str::diff("? loading"));
}

/// キャッシュミス後、daemon が内部でリフレッシュ完了 → prompt がキャッシュから出力を返す
#[test]
fn test_daemon_fresh_returns_cached_output() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));
    let _daemon = DaemonHandle::start(&env);

    // 初回クエリ: キャッシュミス → daemon が内部スレッドでリフレッシュを実行
    let mut cmd = Command::cargo_bin(BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert()
        .success()
        .stdout(predicate::str::diff("? loading"));

    // daemon 内部リフレッシュ完了をポーリングで待つ（最大 2000ms）
    DaemonHandle::wait_for_cache(&env, 5000);

    // 壊れた gh を使っても daemon キャッシュからヒットすること
    let broken_env = TestEnv::with_error("gh is broken", 1);
    let mut cmd = Command::cargo_bin(BIN).unwrap();
    cmd.env("PATH", broken_env.path_env());
    cmd.env("HOME", env.home());
    cmd.env("TMPDIR", env.home());
    cmd.current_dir(env.repo_dir.path());
    cmd.arg("prompt");
    cmd.assert()
        .success()
        .stdout(predicate::str::diff("✓ merge-ready"));
}

/// TTL 超過後も stale 値を返す（daemon が内部でリフレッシュを予約）
#[test]
fn test_daemon_stale_returns_output() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));
    // TTL=0 で起動し、キャッシュが常に stale になるようにする
    let _daemon = DaemonHandle::start_with_env(&env, &[("MERGE_READY_STALE_TTL", "0")]);

    // 初回クエリ: キャッシュミス → daemon が内部スレッドでリフレッシュを実行
    let mut cmd = Command::cargo_bin(BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert()
        .success()
        .stdout(predicate::str::diff("? loading"));

    // daemon 内部リフレッシュ完了をポーリングで待つ（最大 2000ms）
    DaemonHandle::wait_for_cache(&env, 5000);

    // TTL=0 なので stale だが、それでも値を返すこと
    let mut cmd = Command::cargo_bin(BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert().success().stdout(predicate::str::contains("✓"));
}

// ── PR なし（main ブランチ等）────────────────────────────────────────────────

/// PR なしブランチで直接実行（--no-cache）→ 何も出力しない
#[test]
fn test_no_pr_direct_shows_nothing() {
    let env = TestEnv::with_no_pr();

    let mut cmd = Command::cargo_bin(BIN).unwrap();
    env.apply(&mut cmd);
    cmd.args(["prompt", "--no-cache"]);
    cmd.assert().success().stdout(predicate::str::is_empty());
}

/// PR なしブランチで daemon 経由: リフレッシュ完了後に「? loading」が消える（issue #88）
#[test]
fn test_daemon_no_pr_shows_nothing_after_refresh() {
    let env = TestEnv::with_no_pr();
    let _daemon = DaemonHandle::start(&env);

    // 初回クエリ: キャッシュミス → ? loading
    let mut cmd = Command::cargo_bin(BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert()
        .success()
        .stdout(predicate::str::diff("? loading"));

    // daemon リフレッシュ完了を待つ（? loading でなくなる ＝ キャッシュ確定）
    DaemonHandle::wait_for_cache(&env, 5000);

    // キャッシュ確定後は何も出力しない（? loading が永続しないことを確認）
    let mut cmd = Command::cargo_bin(BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert().success().stdout(predicate::str::is_empty());
}

/// no-PR の stale リフレッシュ中でも `? loading` に戻らず空出力を維持する。
///
/// 回帰シナリオ:
/// - キャッシュ済み空文字（no-PR）で TTL=0 により stale 化
/// - 1回目の stale クエリで daemon refresh を開始
/// - refresh 実行中の 2回目 stale クエリでも空出力を返すべき
#[test]
fn test_daemon_no_pr_stale_while_refreshing_keeps_empty_output() {
    // Give enough refresh delay to reliably keep the daemon in refreshing state
    // while we issue multiple stale queries below.
    let env = TestEnv::with_no_pr_delay_ms(1000);
    let _daemon = DaemonHandle::start_with_env(&env, &[("MERGE_READY_STALE_TTL", "0")]);

    // 初回クエリ: キャッシュミス -> loading
    let mut cmd = Command::cargo_bin(BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert()
        .success()
        .stdout(predicate::str::diff("? loading"));

    // 初回リフレッシュ完了で空キャッシュ確定
    DaemonHandle::wait_for_cache(&env, 5000);

    // stale 1回目: リフレッシュ開始しつつ空出力
    let mut cmd = Command::cargo_bin(BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert().success().stdout(predicate::str::is_empty());

    // stale 2回目以降（refresh実行中を狙う）: loading に戻らず空出力を維持
    // 1回だけだと refresh 完了後でも偽陽性になりうるため、短間隔で複数回検証する。
    for _ in 0..5 {
        let mut cmd = Command::cargo_bin(BIN).unwrap();
        env.apply_with_cache(&mut cmd);
        cmd.assert().success().stdout(predicate::str::is_empty());
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

// ── git リポジトリ外 ────────────────────────────────────────────────────

/// git リポジトリでない場合、何も出力しない
#[test]
fn test_no_git_remote_shows_nothing() {
    let env = TestEnv::without_git_remote();

    let mut cmd = Command::cargo_bin(BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert().success().stdout(predicate::str::is_empty());
}

// ── 複数リポジトリの分離 ────────────────────────────────────────────────

/// 同一 daemon が複数リポジトリを正しく分離してキャッシュすること。
///
/// repo_a（merge-ready）と repo_b（conflict）が同じ daemon を共有するとき、
/// daemon は各エントリの cwd で `gh` を実行するため、互いの出力を汚染しない。
/// fake gh は `$PWD/.gh_pr_view.json` を読むため、cwd が正しくなければ
/// repo_b のリフレッシュが repo_a のレスポンスを返してしまうことで検出できる。
#[test]
fn test_daemon_multi_repo_isolation() {
    let env = MultiRepoEnv::new(
        // repo_a: merge-ready
        r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null}"#,
        // repo_b: conflict
        r#"{"state":"OPEN","isDraft":false,"mergeable":"CONFLICTING","mergeStateStatus":"DIRTY","reviewDecision":null}"#,
    );
    let _daemon = env.start_daemon();

    // 両リポジトリのキャッシュが揃うまで待つ
    env.wait_for_cache_in(&env.repo_a, 5000);
    env.wait_for_cache_in(&env.repo_b, 5000);

    let out_a = env.prompt_output(&env.repo_a);
    let out_b = env.prompt_output(&env.repo_b);

    assert_eq!(out_a, "✓ merge-ready", "repo_a should be merge-ready");
    assert!(
        out_b.contains("conflict"),
        "repo_b should show conflict, got: {out_b}"
    );
    assert_ne!(out_a, out_b, "repos must not share cached output");
}

/// daemon version が不一致のとき、prompt 実行で自動再起動される。
#[test]
fn test_prompt_restarts_daemon_on_version_mismatch() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));
    let _old =
        DaemonHandle::start_with_env(&env, &[("MERGE_READY_DAEMON_VERSION_OVERRIDE", "0.0.0")]);

    // 古い daemon が応答することを確認
    let mut before = Command::cargo_bin(BIN).unwrap();
    env.apply(&mut before);
    before
        .args(["daemon", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("version=0.0.0"));

    // prompt 実行で version mismatch を検知し、自動再起動する
    let mut prompt = Command::cargo_bin(BIN).unwrap();
    env.apply_with_cache(&mut prompt);
    prompt.assert().success();

    // 再起動後は実バージョンが返る
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
}
