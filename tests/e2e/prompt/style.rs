//! スタイル構文（`[text](style)`）の E2E テスト

const PROMPT_BIN: &str = "merge-ready-prompt";
const MERGE_READY_JSON: &str = r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":"APPROVED"}"#;
const CHECKS_PASS_JSON: &str = r#"[{"bucket":"pass","state":"SUCCESS"}]"#;

use assert_cmd::Command;
use predicates::prelude::*;

use super::super::helpers::{DaemonHandle, TestEnv};

/// スタイル構文付き format を設定した場合、ANSI エスケープコードが出力に含まれる。
#[test]
fn styled_format_produces_ansi_in_output() {
    let env = TestEnv::new(MERGE_READY_JSON, Some(CHECKS_PASS_JSON));
    env.write_config("[merge_ready]\nformat = \"[$symbol](bold green) $label\"");

    let _daemon = DaemonHandle::start(&env);
    DaemonHandle::wait_for_cache(&env, 5000);

    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\x1b["))
        .stderr("");
}

/// スタイル構文なしの format（デフォルト）では ANSI コードを出力しない（後方互換）。
#[test]
fn plain_format_produces_no_ansi() {
    let env = TestEnv::new(MERGE_READY_JSON, Some(CHECKS_PASS_JSON));

    let _daemon = DaemonHandle::start(&env);
    DaemonHandle::wait_for_cache(&env, 5000);

    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert()
        .success()
        .stdout(predicate::str::diff("✓ Ready for merge"))
        .stderr("");
}
