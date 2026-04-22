//! エラー系 E2E テスト（シナリオ #34–41）
//!
//! `gh` CLI の各エラーシナリオに対して正しい `stdout` が返ることを検証する。
//! daemon はエラー状態をキャッシュしないため、Direct パス（`--no-cache`）で検証する。
//! 各テストは独立した `TestEnv`（`bin_dir` + `home_dir`）を持つため、
//! 並列実行時に `error.log` が競合しない。

use assert_cmd::Command;

use super::super::helpers::TestEnv;

const BIN: &str = "merge-ready";

fn cmd(env: &TestEnv) -> Command {
    let mut c = Command::cargo_bin(BIN).unwrap();
    env.apply(&mut c);
    c.args(["prompt", "--no-cache"]);
    c
}

// ── #34–36: 認証エラー ────────────────────────────────────────────────────────

/// #34: `gh` バイナリが `PATH` に存在しない → `! gh auth login`
#[test]
fn test_gh_not_installed() {
    let env = TestEnv::without_gh();
    cmd(&env)
        .assert()
        .success()
        .stdout("! gh auth login")
        .stderr("");
}

/// #35: `gh` が `exit code 4` を返す（未ログイン）→ `! gh auth login`
#[test]
fn test_gh_not_logged_in() {
    let env = TestEnv::with_error(
        "To get started with GitHub CLI, please run:  gh auth login",
        4,
    );
    cmd(&env)
        .assert()
        .success()
        .stdout("! gh auth login")
        .stderr("");
}

/// #36: `gh` が `exit 1` + `HTTP 401: Bad credentials` → `! gh auth login`
#[test]
fn test_bad_credentials() {
    let env = TestEnv::with_error(
        "HTTP 401: Bad credentials (https://api.github.com/graphql)",
        1,
    );
    cmd(&env)
        .assert()
        .success()
        .stdout("! gh auth login")
        .stderr("");
}

// ── #37–38: API エラー ────────────────────────────────────────────────────────

/// #37: `gh` が `exit 1` + `HTTP 500` → `✗ api-error`
#[test]
fn test_api_error() {
    let env = TestEnv::with_error("HTTP 500: Internal Server Error", 1);
    cmd(&env)
        .assert()
        .success()
        .stdout("✗ api-error")
        .stderr("");
}

/// #38: `gh` が `exit 1` + `connection refused` → `✗ api-error`
#[test]
fn test_no_network() {
    let env = TestEnv::with_error(
        r#"Post "https://api.github.com/graphql": dial tcp: connection refused"#,
        1,
    );
    cmd(&env)
        .assert()
        .success()
        .stdout("✗ api-error")
        .stderr("");
}

// ── #39: レート制限 ───────────────────────────────────────────────────────────

/// #39: `gh` が `exit 1` + `API rate limit exceeded` → `✗ rate-limited`
#[test]
fn test_rate_limited() {
    let env = TestEnv::with_error(
        "HTTP 403: API rate limit exceeded (https://api.github.com/graphql)",
        1,
    );
    cmd(&env)
        .assert()
        .success()
        .stdout("✗ rate-limited")
        .stderr("");
}

// ── #40: タイムアウト ─────────────────────────────────────────────────────────

/// #40: `gh` がハングした場合、タイムアウト後に `✗ api-error` を返すこと。
#[test]
fn test_gh_timeout() {
    let env = TestEnv::with_hanging_gh();
    cmd(&env)
        .env("MERGE_READY_GH_TIMEOUT_SECS", "2")
        .timeout(std::time::Duration::from_secs(5))
        .assert()
        .success()
        .stdout("✗ api-error");
}

// ── #41: エラーログ ───────────────────────────────────────────────────────────

/// #41: API エラー発生時に `$HOME/.cache/merge-ready/error.log` へ追記されること。
#[test]
fn test_error_log_written() {
    let env = TestEnv::with_error("HTTP 500: Internal Server Error", 1);
    let log_path = env.home().join(".cache/merge-ready/error.log");

    cmd(&env).assert().success();

    assert!(log_path.exists(), "error.log が作成されていない");
    let content = std::fs::read_to_string(&log_path).unwrap();
    assert!(!content.is_empty(), "error.log が空");
}
