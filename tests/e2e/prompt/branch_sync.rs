//! ブランチ同期状態の E2E テスト（シナリオ #17–22）
//!
//! 対象条件: `conflict` / `update-branch` / `sync-unknown`
//! 実行フローは daemon 経由（`merge-ready prompt`）に統一する。

use assert_cmd::Command;
use predicates::prelude::*;
use rstest::rstest;

use super::super::helpers::{DaemonHandle, TestEnv};

const BIN: &str = "merge-ready";

const CONFLICTING_DIRTY: &str = r#"{"state":"OPEN","isDraft":false,"mergeable":"CONFLICTING","mergeStateStatus":"DIRTY","reviewDecision":null}"#;
const CONFLICTING_BEHIND: &str = r#"{"state":"OPEN","isDraft":false,"mergeable":"CONFLICTING","mergeStateStatus":"BEHIND","reviewDecision":null}"#;
const CONFLICTING_DIRTY_CHANGES: &str = r#"{"state":"OPEN","isDraft":false,"mergeable":"CONFLICTING","mergeStateStatus":"DIRTY","reviewDecision":"CHANGES_REQUESTED"}"#;
const MERGEABLE_BLOCKED: &str = r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"BLOCKED","reviewDecision":null,"baseRefName":"main","headRefName":"feat/test"}"#;

const PASS_JSON: &str = r#"[{"bucket":"pass","state":"SUCCESS"}]"#;
const FAIL_JSON: &str = r#"[{"bucket":"fail","state":"FAILURE"}]"#;

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

// ── #17, #20–22: conflict 系 ──────────────────────────────────────────────────

/// #17 `CONFLICTING` → `✗ conflict`
/// #20 `CONFLICTING` + `BEHIND` → `conflict` のみ（`update-branch` は抑制）
/// #21 `conflict` + `ci-fail` → 両方をスペース区切りで出力
/// #22 `conflict` + `review` → 両方をスペース区切りで出力
#[rstest]
#[case::conflict(CONFLICTING_DIRTY, PASS_JSON, "✗ conflict")]
#[case::conflict_wins_over_update_branch(CONFLICTING_BEHIND, PASS_JSON, "✗ conflict")]
#[case::conflict_and_ci_fail(CONFLICTING_DIRTY, FAIL_JSON, "✗ conflict ✗ ci-fail")]
#[case::conflict_and_review(CONFLICTING_DIRTY_CHANGES, PASS_JSON, "✗ conflict ⚠ review")]
fn test_conflict_prompt(#[case] pr_json: &str, #[case] checks_json: &str, #[case] expected: &str) {
    let env = TestEnv::new(pr_json, Some(checks_json));
    assert_prompt(&env, expected);
}

// ── #18: update-branch ────────────────────────────────────────────────────────

/// #18: compare API の `behind_by > 0` → `✗ update-branch`
#[test]
fn test_update_branch() {
    let env = TestEnv::with_behind_by(MERGEABLE_BLOCKED, Some(PASS_JSON), 1);
    assert_prompt(&env, "✗ update-branch");
}

// ── #19: sync-unknown ────────────────────────────────────────────────────────

/// #19: compare API がエラーを返す → `? sync-unknown`
#[test]
fn test_compare_api_error() {
    let env = TestEnv::with_compare_error(MERGEABLE_BLOCKED, Some(PASS_JSON));
    assert_prompt(&env, "? sync-unknown");
}
