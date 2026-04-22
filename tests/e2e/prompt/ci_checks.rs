//! CI チェックの E2E テスト（シナリオ #23–29）
//!
//! 対象条件: `ci-fail` / `ci-action`（`gh pr checks --json bucket,state` の結果）
//! 実行フローは daemon 経由（`merge-ready prompt`）に統一する。

use assert_cmd::Command;
use predicates::prelude::*;
use rstest::rstest;

use super::super::helpers::{DaemonHandle, TestEnv};

const BIN: &str = "merge-ready";

const BLOCKED_NO_REVIEW: &str = r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"BLOCKED","reviewDecision":null}"#;
const BLOCKED_CHANGES_REQUESTED: &str = r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"BLOCKED","reviewDecision":"CHANGES_REQUESTED"}"#;
const APPROVED_CLEAN: &str = r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":"APPROVED"}"#;

const FAIL_JSON: &str = r#"[{"bucket":"fail","state":"FAILURE"}]"#;
const CANCEL_JSON: &str = r#"[{"bucket":"cancel","state":"CANCELLED"}]"#;
const ACTION_REQUIRED_JSON: &str = r#"[{"bucket":"action_required","state":"ACTION_REQUIRED"}]"#;
const FAIL_AND_ACTION_JSON: &str = r#"[{"bucket":"fail","state":"FAILURE"},{"bucket":"action_required","state":"ACTION_REQUIRED"}]"#;

fn assert_prompt(env: &TestEnv, expected: &str) {
    let _daemon = DaemonHandle::start(env);
    DaemonHandle::wait_for_cache(env, 5000);

    let mut cmd = Command::cargo_bin(BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert()
        .success()
        .stdout(predicate::str::diff(expected.to_owned()))
        .stderr("");
}

// ── #23–26, #29: checks bucket ────────────────────────────────────────────────

/// #23 `fail` / #24 `cancel` → `✗ ci-fail`
/// #25 `action_required` → `⚠ ci-action`
/// #26 `fail` + `action_required` 混在 → `✗ ci-fail`（`ci-action` は抑制）
/// #29 `ci-fail` + `review` → 両方をスペース区切りで出力
#[rstest]
#[case::ci_fail_failure(BLOCKED_NO_REVIEW, FAIL_JSON, "✗ ci-fail")]
#[case::ci_fail_cancelled(BLOCKED_NO_REVIEW, CANCEL_JSON, "✗ ci-fail")]
#[case::ci_action(BLOCKED_NO_REVIEW, ACTION_REQUIRED_JSON, "⚠ ci-action")]
#[case::ci_fail_wins_over_ci_action(BLOCKED_NO_REVIEW, FAIL_AND_ACTION_JSON, "✗ ci-fail")]
#[case::ci_fail_and_review(BLOCKED_CHANGES_REQUESTED, FAIL_JSON, "✗ ci-fail ⚠ review")]
fn test_ci_check_prompt(#[case] pr_json: &str, #[case] checks_json: &str, #[case] expected: &str) {
    let env = TestEnv::new(pr_json, Some(checks_json));
    assert_prompt(&env, expected);
}

// ── #27–28: CI 未設定 ─────────────────────────────────────────────────────────

/// #27 `no checks reported` + review なし → `✓ merge-ready`
/// #28 `no checks reported` + review あり → `⚠ review`
#[rstest]
#[case::merge_ready(APPROVED_CLEAN, "✓ merge-ready")]
#[case::review(BLOCKED_CHANGES_REQUESTED, "⚠ review")]
fn test_no_ci_checks_prompt(#[case] pr_json: &str, #[case] expected: &str) {
    let env = TestEnv::with_no_ci_checks(pr_json);
    assert_prompt(&env, expected);
}
