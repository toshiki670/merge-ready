//! CLI サブコマンドの E2E テスト
//!
//! `merge-ready prompt` / `merge-ready help` / フラグ類の動作を検証する。

use super::helpers::TestEnv;
use assert_cmd::Command;

const MERGE_READY_PR_VIEW_JSON: &str = r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":"APPROVED"}"#;
const MERGE_READY_PR_CHECKS_JSON: &str = r#"[{"bucket":"pass","state":"SUCCESS"}]"#;

fn cmd(env: &TestEnv) -> Command {
    let mut c = Command::cargo_bin("merge-ready").unwrap();
    env.apply(&mut c);
    c
}

/// 引数なし → ヘルプを表示する
#[test]
fn test_default_no_args_shows_help() {
    let env = TestEnv::new(MERGE_READY_PR_VIEW_JSON, Some(MERGE_READY_PR_CHECKS_JSON));
    cmd(&env)
        .assert()
        .success()
        .stdout(predicates::str::contains("Usage:"));
}

/// `prompt` サブコマンド → 引数なしと同一の出力
#[test]
fn test_prompt_subcommand() {
    let env = TestEnv::new(MERGE_READY_PR_VIEW_JSON, Some(MERGE_READY_PR_CHECKS_JSON));
    cmd(&env)
        .arg("prompt")
        .assert()
        .success()
        .stdout("✓ merge-ready");
}

/// `help` サブコマンド → "Usage:" を含む / exit 0
#[test]
fn test_help_subcommand() {
    let env = TestEnv::new(MERGE_READY_PR_VIEW_JSON, Some(MERGE_READY_PR_CHECKS_JSON));
    cmd(&env)
        .arg("help")
        .assert()
        .success()
        .stdout(predicates::str::contains("Usage:"));
}

/// `--help` フラグ → "Usage:" を含む / exit 0
#[test]
fn test_help_flag_long() {
    let env = TestEnv::new(MERGE_READY_PR_VIEW_JSON, Some(MERGE_READY_PR_CHECKS_JSON));
    cmd(&env)
        .arg("--help")
        .assert()
        .success()
        .stdout(predicates::str::contains("Usage:"));
}

/// `-h` フラグ → "Usage:" を含む / exit 0
#[test]
fn test_help_flag_short() {
    let env = TestEnv::new(MERGE_READY_PR_VIEW_JSON, Some(MERGE_READY_PR_CHECKS_JSON));
    cmd(&env)
        .arg("-h")
        .assert()
        .success()
        .stdout(predicates::str::contains("Usage:"));
}

/// `--version` フラグ → バージョン文字列を含む / exit 0
#[test]
fn test_version_flag() {
    let env = TestEnv::new(MERGE_READY_PR_VIEW_JSON, Some(MERGE_READY_PR_CHECKS_JSON));
    cmd(&env)
        .arg("--version")
        .assert()
        .success()
        .stdout(predicates::str::contains(env!("CARGO_PKG_VERSION")));
}

/// 未知の引数 → exit 非ゼロ
#[test]
fn test_unknown_arg_fails() {
    let env = TestEnv::new(MERGE_READY_PR_VIEW_JSON, Some(MERGE_READY_PR_CHECKS_JSON));
    cmd(&env).arg("unknown").assert().failure();
}
