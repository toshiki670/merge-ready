//! エラー系 E2E テスト（シナリオ #34–41）
//!
//! `gh` CLI の各エラーシナリオに対して正しい `stdout` が返ることを検証する。
//! daemon がエラー状態をキャッシュするため、daemon 経由（キャッシュパス）で検証する。
//! 各テストは独立した `TestEnv`（`bin_dir` + `home_dir`）を持つため、
//! 並列実行時に `error.log` が競合しない。

use assert_cmd::Command;
use rstest::rstest;

use super::super::helpers::{DaemonHandle, TestEnv};

const BIN: &str = "merge-ready";

fn cmd(env: &TestEnv) -> Command {
    let mut c = Command::cargo_bin(BIN).unwrap();
    env.apply_with_cache(&mut c);
    c
}

// ── #34: gh が PATH に存在しない ──────────────────────────────────────────────

/// #34: `gh` バイナリが `PATH` に存在しない → `! gh auth login`
#[test]
fn test_gh_not_installed() {
    let env = TestEnv::without_gh();
    let _daemon = DaemonHandle::start(&env);
    DaemonHandle::wait_for_cache(&env, 5000);

    cmd(&env)
        .assert()
        .success()
        .stdout("! gh auth login")
        .stderr("");
}

// ── #35–39: with_error() 系 ───────────────────────────────────────────────────

/// #35 `exit 4`（未ログイン）/ #36 `HTTP 401`（認証エラー）→ `! gh auth login`
/// #37 `HTTP 500` / #38 `connection refused` → `✗ api-error`
/// #39 `HTTP 403`（レート制限）→ `✗ rate-limited`
#[rstest]
#[case::not_logged_in(
    "To get started with GitHub CLI, please run:  gh auth login",
    4,
    "! gh auth login"
)]
#[case::bad_credentials(
    "HTTP 401: Bad credentials (https://api.github.com/graphql)",
    1,
    "! gh auth login"
)]
#[case::api_error("HTTP 500: Internal Server Error", 1, "✗ api-error")]
#[case::no_network(
    r#"Post "https://api.github.com/graphql": dial tcp: connection refused"#,
    1,
    "✗ api-error"
)]
#[case::rate_limited(
    "HTTP 403: API rate limit exceeded (https://api.github.com/graphql)",
    1,
    "✗ rate-limited"
)]
fn test_error_output(#[case] msg: &str, #[case] code: u8, #[case] expected: &str) {
    let env = TestEnv::with_error(msg, code);
    let _daemon = DaemonHandle::start(&env);
    DaemonHandle::wait_for_cache(&env, 5000);

    cmd(&env)
        .assert()
        .success()
        .stdout(expected.to_owned())
        .stderr("");
}

// ── #40: タイムアウト ─────────────────────────────────────────────────────────

/// #40: `gh` がハングした場合、タイムアウト後に `✗ api-error` を返すこと。
#[test]
fn test_gh_timeout() {
    let env = TestEnv::with_hanging_gh();
    let _daemon = DaemonHandle::start_with_env(&env, &[("MERGE_READY_GH_TIMEOUT_SECS", "2")]);
    DaemonHandle::wait_for_cache(&env, 10000);

    cmd(&env).assert().success().stdout("✗ api-error");
}

// ── #41: エラーログ ───────────────────────────────────────────────────────────

/// #41: API エラー発生時に `$HOME/.cache/merge-ready/error.log` へ追記されること。
#[test]
fn test_error_log_written() {
    let env = TestEnv::with_error("HTTP 500: Internal Server Error", 1);
    let log_path = env.home().join(".cache/merge-ready/error.log");
    let _daemon = DaemonHandle::start(&env);
    DaemonHandle::wait_for_cache(&env, 5000);

    cmd(&env).assert().success();

    assert!(log_path.exists(), "error.log が作成されていない");
    let content = std::fs::read_to_string(&log_path).unwrap();
    assert!(!content.is_empty(), "error.log が空");
}
