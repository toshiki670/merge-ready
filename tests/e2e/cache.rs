//! キャッシュ機構の e2e テスト
//!
//! これらのテストは `TestEnv::apply_with_cache()` を使用してキャッシュを有効にする。
//! `home_dir` は tempdir で分離されているため、テスト間でキャッシュはリークしない。

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use super::helpers::TestEnv;

/// merge-ready のバイナリ名
const BIN: &str = "merge-ready";

/// fake git が返すワークツリーパス（`git rev-parse --show-toplevel` の戻り値）
const FAKE_TOPLEVEL: &str = "/fake/repo";

/// FNV-1a ハッシュで生成される repo_id（`infra::repo_id::path_to_id` と同じアルゴリズム）
fn fake_repo_id() -> String {
    let mut hash: u64 = 14_695_981_039_346_656_037;
    for byte in FAKE_TOPLEVEL.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(1_099_511_628_211);
    }
    format!("{hash:016x}")
}

/// マージ可能な PR の `gh pr view` JSON（キャッシュテスト用の最小セット）
/// `baseRefName` / `headRefName` を空にすることで `gh repo view` 呼び出しを回避し
/// ブランチ同期を "Clean" として扱う
const OPEN_PR_VIEW_JSON: &str = r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null}"#;

/// CI が Pass の `gh pr checks` JSON
const CI_PASS_JSON: &str = r#"[{"bucket":"pass","state":"SUCCESS","name":"ci","link":""}]"#;

// ── キャッシュなし（初回起動） ──────────────────────────────────────────

/// キャッシュなし → `? loading` を出力する
#[test]
fn test_cache_miss_shows_loading() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));

    let mut cmd = Command::cargo_bin(BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert()
        .success()
        .stdout(predicate::str::diff("? loading"));
}

// ── --refresh モード ─────────────────────────────────────────────────────

/// `--refresh` モードを明示的に実行すると state.json が作成される
#[test]
fn test_refresh_mode_writes_cache() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));

    let mut cmd = Command::cargo_bin(BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.arg("--refresh");
    // prompt --refresh は stdout に何も出力しない
    cmd.assert().success().stdout(predicate::str::is_empty());

    // state.json が作成されていること
    let state_path = state_json_path(env.home());
    assert!(
        state_path.exists(),
        "state.json should be created by --refresh at: {}",
        state_path.display()
    );

    // state.json の内容確認
    let state = read_state_json(env.home()).expect("state.json should be parseable");
    let fetched_at: u64 = state["fetched_at_secs"]
        .as_u64()
        .expect("fetched_at_secs should be u64");
    assert!(
        fetched_at > now_secs() - 5,
        "fetched_at_secs should be recent (was: {fetched_at})"
    );
}

// ── キャッシュヒット（新鮮） ────────────────────────────────────────────

/// 新鮮な state.json が存在する場合、キャッシュの値がそのまま返る
#[test]
fn test_cache_fresh_returns_cached_output() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));
    let cached_output = "✓ merge-ready";

    // state.json を手動で書き込む（fetched_at_secs = now）
    write_state_json(env.home(), cached_output, now_secs());

    // 壊れた fake gh を使っても、キャッシュが使われるためエラーにならない
    let broken_env = TestEnv::with_error("gh is broken", 1);
    let mut cmd = Command::cargo_bin(BIN).unwrap();
    cmd.env("PATH", broken_env.path_env()); // 壊れた gh の PATH
    cmd.env("HOME", env.home()); // キャッシュが存在する HOME
    cmd.arg("prompt"); // キャッシュ有効モード

    cmd.assert()
        .success()
        .stdout(predicate::str::diff(cached_output));
}

// ── キャッシュ stale（期限切れ） ───────────────────────────────────────

/// stale_ttl より古いキャッシュが存在する場合、キャッシュの値を出力する
#[test]
fn test_cache_stale_returns_cached_output() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));
    let cached_output = "✗ conflict";

    // stale_ttl(5秒)より古いキャッシュを作成: now - 10秒
    write_state_json(env.home(), cached_output, now_secs() - 10);

    let mut cmd = Command::cargo_bin(BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    // stale でもキャッシュ値を返す（ブロックしない）
    cmd.assert()
        .success()
        .stdout(predicate::str::diff(cached_output));
}

/// stale 後に `--refresh` を実行すると state.json が更新される
#[test]
fn test_stale_cache_is_updated_by_refresh() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));
    let stale_output = "✗ conflict";

    // stale_ttl より古いキャッシュを作成
    write_state_json(env.home(), stale_output, now_secs() - 10);

    // prompt --refresh を明示的に実行
    let mut refresh_cmd = Command::cargo_bin(BIN).unwrap();
    env.apply_with_cache(&mut refresh_cmd);
    refresh_cmd.arg("--refresh");
    refresh_cmd
        .assert()
        .success()
        .stdout(predicate::str::is_empty());

    // state.json の fetched_at_secs が更新されていること
    let state = read_state_json(env.home()).expect("state.json should exist");
    let new_fetched_at: u64 = state["fetched_at_secs"]
        .as_u64()
        .expect("fetched_at_secs should be u64");
    assert!(
        new_fetched_at > now_secs() - 5,
        "fetched_at_secs should be updated by --refresh (was: {new_fetched_at})"
    );
}

// ── git remote 取得不可 ─────────────────────────────────────────────────

/// git リポジトリでない場合、何も出力せずキャッシュも作らない
#[test]
fn test_no_git_remote_shows_nothing() {
    let env = TestEnv::without_git_remote();

    let mut cmd = Command::cargo_bin(BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert().success().stdout(predicate::str::is_empty());

    // キャッシュが作成されていないこと
    let state_path = state_json_path(env.home());
    assert!(
        !state_path.exists(),
        "state.json should NOT be created when not in a git repository"
    );
}

// ── ヘルパー関数 ────────────────────────────────────────────────────────

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn state_json_path(home: &std::path::Path) -> std::path::PathBuf {
    home.join(".cache")
        .join("merge-ready")
        .join(format!("{}.json", fake_repo_id()))
}

/// 指定した home_dir の下に state.json を書き込む
fn write_state_json(home: &std::path::Path, output: &str, fetched_at_secs: u64) {
    let state_path = state_json_path(home);
    if let Some(parent) = state_path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let content = format!(r#"{{"fetched_at_secs":{fetched_at_secs},"output":"{output}"}}"#);
    fs::write(&state_path, content).unwrap();
}

fn read_state_json(home: &std::path::Path) -> Option<serde_json::Value> {
    let content = fs::read_to_string(state_json_path(home)).ok()?;
    serde_json::from_str(&content).ok()
}
