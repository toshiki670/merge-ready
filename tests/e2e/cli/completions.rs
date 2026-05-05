use assert_cmd::Command;

use super::super::helpers::TestEnv;

const MERGE_READY_PR_VIEW_JSON: &str = r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":"APPROVED"}"#;
const MERGE_READY_PR_CHECKS_JSON: &str = r#"[{"bucket":"pass","state":"SUCCESS"}]"#;

const BIN: &str = "merge-ready";

fn cmd(env: &TestEnv) -> Command {
    let mut c = Command::cargo_bin(BIN).unwrap();
    env.apply(&mut c);
    c
}

/// `completions bash` → 成功し、bash 補完スクリプトを stdout に出力する
#[test]
fn test_completions_bash() {
    let env = TestEnv::new(MERGE_READY_PR_VIEW_JSON, Some(MERGE_READY_PR_CHECKS_JSON));
    cmd(&env)
        .args(["completions", "bash"])
        .assert()
        .success()
        .stdout(predicates::str::contains("merge-ready"));
}

/// `completions zsh` → 成功し、zsh 補完スクリプトを stdout に出力する
#[test]
fn test_completions_zsh() {
    let env = TestEnv::new(MERGE_READY_PR_VIEW_JSON, Some(MERGE_READY_PR_CHECKS_JSON));
    cmd(&env)
        .args(["completions", "zsh"])
        .assert()
        .success()
        .stdout(predicates::str::contains("merge-ready"));
}

/// `completions fish` → 成功し、fish 補完スクリプトを stdout に出力する
#[test]
fn test_completions_fish() {
    let env = TestEnv::new(MERGE_READY_PR_VIEW_JSON, Some(MERGE_READY_PR_CHECKS_JSON));
    cmd(&env)
        .args(["completions", "fish"])
        .assert()
        .success()
        .stdout(predicates::str::contains("merge-ready"));
}

/// `completions` (シェル引数なし) → 失敗する
#[test]
fn test_completions_no_shell_fails() {
    let env = TestEnv::new(MERGE_READY_PR_VIEW_JSON, Some(MERGE_READY_PR_CHECKS_JSON));
    cmd(&env).arg("completions").assert().failure();
}

/// `completions unknown-shell` → 失敗する
#[test]
fn test_completions_unknown_shell_fails() {
    let env = TestEnv::new(MERGE_READY_PR_VIEW_JSON, Some(MERGE_READY_PR_CHECKS_JSON));
    cmd(&env)
        .args(["completions", "unknown-shell"])
        .assert()
        .failure();
}
