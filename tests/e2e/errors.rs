//! エラー系 E2E テスト
//!
//! `gh` CLI の各エラーシナリオに対して正しい `stdout` が返ることを検証する。
//! エラー判別仕様は dotfiles#282 の実測結果に基づく:
//!   - spawn 失敗（`gh` 未インストール）  → `! gh auth login`
//!   - `exit 4`（未ログイン）            → `! gh auth login`
//!   - `exit 1` + `HTTP 401`            → `! gh auth login`
//!   - `exit 1` + `no pull requests`    → `""` (空)
//!   - `exit 1` + `rate limit`          → `✗ rate-limited`
//!   - `exit 1` + その他                → `✗ api-error`
//!
//! 各テストは独立した `TestEnv`（`bin_dir` + `home_dir`）を持つため、
//! 並列実行時に `~/.cache/ci-status/error.log` が競合しない。

use super::helpers::TestEnv;
use assert_cmd::Command;

// ─── 認証エラー ───────────────────────────────────────────────────────────

/// `gh` バイナリが `PATH` に存在しない → `! gh auth login`
#[test]
fn test_gh_not_installed() {
    let env = TestEnv::without_gh();
    let mut cmd = Command::cargo_bin("merge-ready").unwrap();
    env.apply(&mut cmd);
    cmd.arg("prompt")
        .assert()
        .success()
        .stdout("! gh auth login")
        .stderr("");
}

/// `gh` が `exit code 4` を返す（未ログイン）→ `! gh auth login`
#[test]
fn test_gh_not_logged_in() {
    let env = TestEnv::with_error(
        "To get started with GitHub CLI, please run:  gh auth login",
        4,
    );
    let mut cmd = Command::cargo_bin("merge-ready").unwrap();
    env.apply(&mut cmd);
    cmd.arg("prompt")
        .assert()
        .success()
        .stdout("! gh auth login")
        .stderr("");
}

/// `gh` が `exit 1` + `HTTP 401: Bad credentials` → `! gh auth login`
#[test]
fn test_bad_credentials() {
    let env = TestEnv::with_error(
        "HTTP 401: Bad credentials (https://api.github.com/graphql)",
        1,
    );
    let mut cmd = Command::cargo_bin("merge-ready").unwrap();
    env.apply(&mut cmd);
    cmd.arg("prompt")
        .assert()
        .success()
        .stdout("! gh auth login")
        .stderr("");
}

// ─── API エラー ───────────────────────────────────────────────────────────

/// `gh` が `exit 1` + `HTTP 500` → `✗ api-error`
#[test]
fn test_api_error() {
    let env = TestEnv::with_error("HTTP 500: Internal Server Error", 1);
    let mut cmd = Command::cargo_bin("merge-ready").unwrap();
    env.apply(&mut cmd);
    cmd.arg("prompt")
        .assert()
        .success()
        .stdout("✗ api-error")
        .stderr("");
}

/// `gh` が `exit 1` + `connection refused`（ネットワーク不通）→ `✗ api-error`
#[test]
fn test_no_network() {
    let env = TestEnv::with_error(
        r#"Post "https://api.github.com/graphql": dial tcp: connection refused"#,
        1,
    );
    let mut cmd = Command::cargo_bin("merge-ready").unwrap();
    env.apply(&mut cmd);
    cmd.arg("prompt")
        .assert()
        .success()
        .stdout("✗ api-error")
        .stderr("");
}

/// `gh` が `exit 1` + `API rate limit exceeded` → `✗ rate-limited`
#[test]
fn test_rate_limited() {
    let env = TestEnv::with_error(
        "HTTP 403: API rate limit exceeded (https://api.github.com/graphql)",
        1,
    );
    let mut cmd = Command::cargo_bin("merge-ready").unwrap();
    env.apply(&mut cmd);
    cmd.arg("prompt")
        .assert()
        .success()
        .stdout("✗ rate-limited")
        .stderr("");
}

// ─── エラーログ ───────────────────────────────────────────────────────────

/// API エラー発生時に `$HOME/.cache/ci-status/error.log` へ追記されること。
/// `TestEnv` の `home_dir` を `HOME` に設定しているため実際の `~` には書き込まれない。
#[test]
fn test_error_log_written() {
    let env = TestEnv::with_error("HTTP 500: Internal Server Error", 1);
    let log_path = env.home().join(".cache/ci-status/error.log");

    let mut cmd = Command::cargo_bin("merge-ready").unwrap();
    env.apply(&mut cmd);
    cmd.arg("prompt").assert().success();

    assert!(log_path.exists(), "error.log が作成されていない");
    let content = std::fs::read_to_string(&log_path).unwrap();
    assert!(!content.is_empty(), "error.log が空");
}
