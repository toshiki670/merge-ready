//! 複数 PR シナリオ: 1ブランチに複数のオープン PR が存在する場合の表示検証（#256）
//!
//! 複数 PR は番号昇順で並び、各 PR の状態を `#<number>` 付きでスペース区切りに表示する。

const PROMPT_BIN: &str = "merge-ready-prompt";

use assert_cmd::Command;
use predicates::prelude::*;

use super::super::helpers::{DaemonHandle, TestEnv};

/// 2つの PR がある場合: 番号昇順で状態を並べて表示する
///
/// PR #200: merge ready (CLEAN, approved)
/// PR #201: draft
/// 期待出力: "✓ Ready for merge #200 ✎ Ready for review #201"
#[test]
fn test_multiple_prs_shown_in_order() {
    let pr_list_json = r#"[
        {"number":200,"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null,"baseRefName":"","headRefName":""},
        {"number":201,"state":"OPEN","isDraft":true,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null,"baseRefName":"","headRefName":""}
    ]"#;
    let env = TestEnv::with_pr_list(pr_list_json, Some(r#"[]"#));

    let _daemon = DaemonHandle::start(&env);
    DaemonHandle::wait_for_cache(&env, 5000);

    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert()
        .success()
        .stdout(predicate::str::diff(
            "✓ Ready for merge #200 ✎ Ready for review #201",
        ))
        .stderr("");
}

/// 複数 PR でブロック状態が混在する場合: 各 PR の状態を独立して表示する
///
/// PR #300: merge ready (CLEAN, CI pass)
/// PR #301: レビュー待ち (BLOCKED, REVIEW_REQUIRED)
/// 期待出力: "✓ Ready for merge #300 @ Assign reviewer #301"
#[test]
fn test_multiple_prs_with_mixed_states() {
    let pr_list_json = r#"[
        {"number":300,"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null,"baseRefName":"","headRefName":""},
        {"number":301,"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"BLOCKED","reviewDecision":"REVIEW_REQUIRED","baseRefName":"","headRefName":""}
    ]"#;
    let env = TestEnv::with_pr_list(pr_list_json, Some(r#"[]"#));

    let _daemon = DaemonHandle::start(&env);
    DaemonHandle::wait_for_cache(&env, 5000);

    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert()
        .success()
        .stdout(predicate::str::diff(
            "✓ Ready for merge #300 @ Assign reviewer #301",
        ))
        .stderr("");
}
