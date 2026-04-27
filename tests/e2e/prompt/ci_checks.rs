//! CI チェックの E2E テスト（シナリオ #23–29）
//!
//! 対象条件: `ci_fail` / `ci_action`（`gh pr checks --json bucket,state` の結果）
//! 実行フローは daemon 経由（`merge-ready prompt`）に統一する。

use assert_cmd::Command;
use predicates::prelude::*;
use rstest::rstest;

use super::super::helpers::{DaemonHandle, TestEnv};

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

    let mut cmd = Command::cargo_bin("merge-ready-prompt").unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert()
        .success()
        .stdout(predicate::str::diff(expected.to_owned()))
        .stderr("");
}

// ── #23–26, #29: checks bucket ────────────────────────────────────────────────

/// #23 `fail` / #24 `cancel` → `✗ Fix CI failure`
/// #25 `action_required` → `⚠ Run CI action`
/// #26 `fail` + `action_required` 混在 → `✗ Fix CI failure`（`Run CI action` は抑制）
/// #29 `Fix CI failure` + `Resolve review` → 両方をスペース区切りで出力
#[rstest]
#[case::ci_fail_failure(BLOCKED_NO_REVIEW, FAIL_JSON, "✗ Fix CI failure")]
#[case::ci_fail_cancelled(BLOCKED_NO_REVIEW, CANCEL_JSON, "✗ Fix CI failure")]
#[case::ci_action(BLOCKED_NO_REVIEW, ACTION_REQUIRED_JSON, "⚠ Run CI action")]
#[case::ci_fail_wins_over_ci_action(BLOCKED_NO_REVIEW, FAIL_AND_ACTION_JSON, "✗ Fix CI failure")]
#[case::ci_fail_and_review(
    BLOCKED_CHANGES_REQUESTED,
    FAIL_JSON,
    "✗ Fix CI failure ⚠ Resolve review"
)]
fn test_ci_check_prompt(#[case] pr_json: &str, #[case] checks_json: &str, #[case] expected: &str) {
    let env = TestEnv::new(pr_json, Some(checks_json));
    assert_prompt(&env, expected);
}

// ── #27–28: CI 未設定 ─────────────────────────────────────────────────────────

/// #27 `no checks reported` + review なし → `✓ Ready for merge`
/// #28 `no checks reported` + review あり → `⚠ Resolve review`
#[rstest]
#[case::merge_ready(APPROVED_CLEAN, "✓ Ready for merge")]
#[case::review(BLOCKED_CHANGES_REQUESTED, "⚠ Resolve review")]
fn test_no_ci_checks_prompt(#[case] pr_json: &str, #[case] expected: &str) {
    let env = TestEnv::with_no_ci_checks(pr_json);
    assert_prompt(&env, expected);
}
