//! CI チェックの E2E テスト（シナリオ #23–29）
//!
//! 対象条件: `ci-fail` / `ci-action`（`gh pr checks --json bucket,state` の結果）
//! 実行フローは daemon 経由（`merge-ready prompt`）に統一する。

use assert_cmd::Command;
use predicates::prelude::*;

use super::super::helpers::{DaemonHandle, TestEnv};

const BIN: &str = "merge-ready";

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

// ── 単体 ──────────────────────────────────────────────────────────────────────

/// #23: `checks bucket == fail` → `✗ ci-fail`
#[test]
fn test_ci_fail_failure() {
    let env = TestEnv::new(
        r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"BLOCKED","reviewDecision":null}"#,
        Some(r#"[{"bucket":"fail","state":"FAILURE"}]"#),
    );
    assert_prompt(&env, "✗ ci-fail");
}

/// #24: `checks bucket == cancel` も `✗ ci-fail` として扱う
#[test]
fn test_ci_fail_cancelled() {
    let env = TestEnv::new(
        r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"BLOCKED","reviewDecision":null}"#,
        Some(r#"[{"bucket":"cancel","state":"CANCELLED"}]"#),
    );
    assert_prompt(&env, "✗ ci-fail");
}

/// #25: `checks bucket == action_required` → `⚠ ci-action`
#[test]
fn test_ci_action() {
    let env = TestEnv::new(
        r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"BLOCKED","reviewDecision":null}"#,
        Some(r#"[{"bucket":"action_required","state":"ACTION_REQUIRED"}]"#),
    );
    assert_prompt(&env, "⚠ ci-action");
}

// ── ci_checks 内の優先度 ──────────────────────────────────────────────────────

/// #26: `fail` と `action_required` が混在 → `ci-fail` のみ表示（`ci-action` は抑制される）
#[test]
fn test_ci_fail_wins_over_ci_action() {
    let env = TestEnv::new(
        r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"BLOCKED","reviewDecision":null}"#,
        Some(
            r#"[{"bucket":"fail","state":"FAILURE"},{"bucket":"action_required","state":"ACTION_REQUIRED"}]"#,
        ),
    );
    assert_prompt(&env, "✗ ci-fail");
}

// ── CI 未設定 ──────────────────────────────────────────────────────────────────

/// #27: `gh pr checks` が "no checks reported" + review なし → `✓ merge-ready`
#[test]
fn test_no_ci_checks_merge_ready() {
    let env = TestEnv::with_no_ci_checks(
        r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":"APPROVED"}"#,
    );
    assert_prompt(&env, "✓ merge-ready");
}

/// #28: `gh pr checks` が "no checks reported" + review あり → `⚠ review`
#[test]
fn test_no_ci_checks_with_review() {
    let env = TestEnv::with_no_ci_checks(
        r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"BLOCKED","reviewDecision":"CHANGES_REQUESTED"}"#,
    );
    assert_prompt(&env, "⚠ review");
}

// ── review との複合出力 ───────────────────────────────────────────────────────

/// #29: `ci-fail` + `review` → 両方をスペース区切りで出力
#[test]
fn test_ci_fail_and_review() {
    let env = TestEnv::new(
        r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"BLOCKED","reviewDecision":"CHANGES_REQUESTED"}"#,
        Some(r#"[{"bucket":"fail","state":"FAILURE"}]"#),
    );
    assert_prompt(&env, "✗ ci-fail ⚠ review");
}
