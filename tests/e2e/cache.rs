//! キャッシュ機構の e2e テスト
//!
//! daemon 経由のキャッシュ動作を検証する。
//! 各テストは独立した `home_dir`（= `TMPDIR`）を持つため、daemon socket が分離される。

use assert_cmd::Command;
use predicates::prelude::*;

use super::helpers::{DaemonHandle, TestEnv};

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

/// `daemon refresh` でキャッシュ投入後 → prompt が daemon キャッシュから出力を返す
#[test]
fn test_daemon_fresh_returns_cached_output() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));
    let _daemon = DaemonHandle::start(&env);

    // daemon refresh でキャッシュを投入する
    let mut refresh = Command::cargo_bin(BIN).unwrap();
    env.apply(&mut refresh);
    refresh.args(["daemon", "refresh", "--repo-id", &env.repo_id()]);
    refresh.assert().success().stdout(predicate::str::is_empty());

    // daemon キャッシュが更新されるまで少し待つ
    std::thread::sleep(std::time::Duration::from_millis(50));

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
    let _daemon = DaemonHandle::start(&env);

    // キャッシュを投入
    let mut refresh = Command::cargo_bin(BIN).unwrap();
    env.apply(&mut refresh);
    refresh.args(["daemon", "refresh", "--repo-id", &env.repo_id()]);
    refresh.assert().success();

    // TTL を 0 に設定して stale を強制、それでも値を返すこと
    let mut cmd = Command::cargo_bin(BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.env("MERGE_READY_STALE_TTL", "0");
    cmd.assert().success().stdout(predicate::str::contains("✓"));
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
