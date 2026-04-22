//! CLI フラグ・サブコマンド共通 E2E テスト（シナリオ #64–72）
//!
//! ヘルプ表示、バージョン表示、未知の引数などの CLI インターフェースを検証する。

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use rstest::rstest;

use super::super::helpers::TestEnv;

const MERGE_READY_PR_VIEW_JSON: &str = r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":"APPROVED"}"#;
const MERGE_READY_PR_CHECKS_JSON: &str = r#"[{"bucket":"pass","state":"SUCCESS"}]"#;

const BIN: &str = "merge-ready";

fn cmd(env: &TestEnv) -> Command {
    let mut c = Command::cargo_bin(BIN).unwrap();
    env.apply(&mut c);
    c
}

// ── #64: 引数なし ─────────────────────────────────────────────────────────────

/// #64: 引数なし → "Usage:" を含むヘルプを表示
#[test]
fn test_default_no_args_shows_help() {
    let env = TestEnv::new(MERGE_READY_PR_VIEW_JSON, Some(MERGE_READY_PR_CHECKS_JSON));
    cmd(&env)
        .assert()
        .success()
        .stdout(predicates::str::contains("Usage:"));
}

// ── #65–67: トップレベルヘルプ ────────────────────────────────────────────────

/// #65 `help` / #66 `--help` / #67 `-h` → "Usage:" を含む・"Output tokens:" を含まない
#[rstest]
#[case::help_subcommand("help")]
#[case::help_long("--help")]
#[case::help_short("-h")]
fn test_top_level_help(#[case] arg: &str) {
    let env = TestEnv::new(MERGE_READY_PR_VIEW_JSON, Some(MERGE_READY_PR_CHECKS_JSON));
    cmd(&env)
        .arg(arg)
        .assert()
        .success()
        .stdout(predicates::str::contains("Usage:"))
        .stdout(predicates::str::contains("Output tokens:").not());
}

// ── #68–70: prompt サブコマンドのヘルプ ──────────────────────────────────────

/// #68 `help prompt` / #69 `prompt --help` / #70 `prompt -h` → "Output tokens:" を含む
#[rstest]
#[case::help_prompt("help", "prompt")]
#[case::prompt_help_long("prompt", "--help")]
#[case::prompt_help_short("prompt", "-h")]
fn test_prompt_help(#[case] arg1: &str, #[case] arg2: &str) {
    let env = TestEnv::new(MERGE_READY_PR_VIEW_JSON, Some(MERGE_READY_PR_CHECKS_JSON));
    cmd(&env)
        .args([arg1, arg2])
        .assert()
        .success()
        .stdout(predicates::str::contains("Output tokens:"));
}

// ── #71: バージョン ───────────────────────────────────────────────────────────

/// #71: `--version` → CARGO_PKG_VERSION を含む
#[test]
fn test_version_flag() {
    let env = TestEnv::new(MERGE_READY_PR_VIEW_JSON, Some(MERGE_READY_PR_CHECKS_JSON));
    cmd(&env)
        .arg("--version")
        .assert()
        .success()
        .stdout(predicates::str::contains(env!("CARGO_PKG_VERSION")));
}

// ── #72: 未知の引数 ───────────────────────────────────────────────────────────

/// #72: 未知の引数 → exit 非ゼロ
#[test]
fn test_unknown_arg_fails() {
    let env = TestEnv::new(MERGE_READY_PR_VIEW_JSON, Some(MERGE_READY_PR_CHECKS_JSON));
    cmd(&env).arg("unknown").assert().failure();
}
