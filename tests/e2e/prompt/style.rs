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

/// `[text](style)` の content 内に conditional ブロックを置いた場合の検証。
///
/// `[$symbol $label( #$pr_id)](cyan)` — 単独 PR（`$pr_id` が空）のとき
/// `( #$pr_id)` ブロックが非表示になり、全体に cyan スタイルが適用される。
#[test]
fn conditional_inside_styled_block_hidden_when_pr_id_empty() {
    let env = TestEnv::new(MERGE_READY_JSON, Some(CHECKS_PASS_JSON));
    env.write_config("[merge_ready]\nformat = \"[$symbol $label( #$pr_id)](cyan)\"");

    let _daemon = DaemonHandle::start(&env);
    DaemonHandle::wait_for_cache(&env, 5000);

    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    // ANSI コードが含まれる（cyan スタイル適用）
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\x1b["))
        .stderr("");

    // `( #)` が出力に含まれないこと（Conditional が非表示）
    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("( #"),
        "conditional block should be hidden, got: {stdout:?}"
    );
    assert!(
        stdout.contains("✓ Ready for merge"),
        "symbol and label should be present, got: {stdout:?}"
    );
    assert!(
        !stdout.contains("( #)"),
        "literal '( #)' must not appear, got: {stdout:?}"
    );
}

/// スタイル適用後のテキストに色が漏れない（reset が挿入される）。
/// `[$symbol](bold green) $label` の $label 部分はデフォルトカラーで出力される。
#[test]
fn text_after_styled_segment_is_not_colored() {
    let env = TestEnv::new(MERGE_READY_JSON, Some(CHECKS_PASS_JSON));
    env.write_config("[merge_ready]\nformat = \"[$symbol](bold green) $label\"");

    let _daemon = DaemonHandle::start(&env);
    DaemonHandle::wait_for_cache(&env, 5000);

    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    // $label ("Ready for merge") がプレーンテキストとして末尾にある。
    cmd.assert()
        .success()
        .stdout(predicate::str::ends_with("Ready for merge"))
        .stderr("");
}
