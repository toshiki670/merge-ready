//! ブランチ同期状態の E2E テスト（シナリオ #17–22）
//!
//! 対象条件: `conflict` / `update_branch` / `sync_unknown`
//! 実行フローは daemon 経由（`merge-ready prompt`）に統一する。

use assert_cmd::Command;
use predicates::prelude::*;
use rstest::rstest;

use super::super::helpers::{DaemonHandle, TestEnv};
use super::branch_sync_fixtures;

const CONFLICTING_DIRTY: &str = r#"{"state":"OPEN","isDraft":false,"mergeable":"CONFLICTING","mergeStateStatus":"DIRTY","reviewDecision":null}"#;
const CONFLICTING_BEHIND: &str = r#"{"state":"OPEN","isDraft":false,"mergeable":"CONFLICTING","mergeStateStatus":"BEHIND","reviewDecision":null}"#;
const CONFLICTING_DIRTY_CHANGES: &str = r#"{"state":"OPEN","isDraft":false,"mergeable":"CONFLICTING","mergeStateStatus":"DIRTY","reviewDecision":"CHANGES_REQUESTED"}"#;
const MERGEABLE_BLOCKED: &str = r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"BLOCKED","reviewDecision":null,"baseRefName":"main","headRefName":"feat/test"}"#;

// #410: compare スキップ検証用。ref を埋めることで「修正前なら compare を呼ぶ」
// 条件を満たし、修正後に呼ばれないことを観測できる。
const CONFLICTING_WITH_REFS: &str = r#"{"state":"OPEN","isDraft":false,"mergeable":"CONFLICTING","mergeStateStatus":"DIRTY","reviewDecision":null,"baseRefName":"main","headRefName":"feat/test"}"#;
const MERGE_STATE_UNKNOWN_WITH_REFS: &str = r#"{"state":"OPEN","isDraft":false,"mergeable":"UNKNOWN","mergeStateStatus":"MERGE_STATE_UNKNOWN","reviewDecision":null,"baseRefName":"main","headRefName":"feat/test"}"#;
const UNKNOWN_WITH_REFS: &str = r#"{"state":"OPEN","isDraft":false,"mergeable":"UNKNOWN","mergeStateStatus":"UNKNOWN","reviewDecision":null,"baseRefName":"main","headRefName":"feat/test"}"#;

// `statusCheckRollup.contexts.nodes` 形式
const PASS_JSON: &str =
    r#"[{"__typename":"CheckRun","status":"COMPLETED","conclusion":"SUCCESS"}]"#;
const FAIL_JSON: &str =
    r#"[{"__typename":"CheckRun","status":"COMPLETED","conclusion":"FAILURE"}]"#;

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

// ── #17, #20–22: conflict 系 ──────────────────────────────────────────────────

/// #17 `CONFLICTING` → `✗ Resolve conflict`
/// #20 `CONFLICTING` + `BEHIND` → `Resolve conflict` のみ（`Update branch` は抑制）
/// #21 `Resolve conflict` + `Fix CI failure` → 両方をスペース区切りで出力
/// #22 `Resolve conflict` + `Resolve review` → 両方をスペース区切りで出力
#[rstest]
#[case::conflict(CONFLICTING_DIRTY, PASS_JSON, "✗ Resolve conflict")]
#[case::conflict_wins_over_update_branch(CONFLICTING_BEHIND, PASS_JSON, "✗ Resolve conflict")]
#[case::conflict_and_ci_fail(CONFLICTING_DIRTY, FAIL_JSON, "✗ Resolve conflict ✗ Fix CI failure")]
#[case::conflict_and_review(
    CONFLICTING_DIRTY_CHANGES,
    PASS_JSON,
    "✗ Resolve conflict ⚠ Resolve review"
)]
fn test_conflict_prompt(#[case] pr_json: &str, #[case] checks_json: &str, #[case] expected: &str) {
    let env = TestEnv::new(pr_json, Some(checks_json));
    assert_prompt(&env, expected);
}

// ── #18: update-branch ────────────────────────────────────────────────────────

/// #18: compare API の `behind_by > 0` → `✗ Update branch`
#[test]
fn test_update_branch() {
    let env = branch_sync_fixtures::with_behind_by(MERGEABLE_BLOCKED, Some(PASS_JSON), 1);
    assert_prompt(&env, "✗ Update branch");
}

// ── #19: sync-unknown ────────────────────────────────────────────────────────

/// #19: compare API がエラーを返す → `? Check branch sync`
#[test]
fn test_compare_api_error() {
    let env = branch_sync_fixtures::with_compare_error(MERGEABLE_BLOCKED, Some(PASS_JSON));
    assert_prompt(&env, "? Check branch sync");
}

/// compare API が exit 0 で不正な JSON を返す → `? Check branch sync`
#[test]
fn test_compare_invalid_json_shows_check_branch_sync() {
    let env = branch_sync_fixtures::with_invalid_compare_json(MERGEABLE_BLOCKED, PASS_JSON);
    assert_prompt(&env, "? Check branch sync");
}

// ── #410: compare スキップ（無駄な REST 呼び出しの抑制） ──────────────────────

/// daemon を起動して最初の refresh 完了を待ち、出力を検証したうえで
/// compare 呼び出しログが生成されていない（= compare ゼロ）ことを確認する。
fn assert_prompt_without_compare(env: &TestEnv, log_path: &std::path::Path, expected: &str) {
    let _daemon = DaemonHandle::start(env);
    DaemonHandle::wait_for_cache(env, 5000);

    let mut cmd = Command::cargo_bin("merge-ready-prompt").unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert()
        .success()
        .stdout(predicate::str::diff(expected.to_owned()))
        .stderr("");

    // refresh が完了してキャッシュが埋まった後に compare ログが無いことは、
    // evaluate_single_pr が compare を await する前に分岐したことを意味する。
    assert!(
        !log_path.exists(),
        "compare API must not be called (found call log at {})",
        log_path.display()
    );
}

/// #410: `CONFLICTING` は `behind_by` を使わない（Conflict 優先）ため compare をスキップする。
#[test]
fn test_conflicting_skips_compare() {
    let (env, log_path) =
        branch_sync_fixtures::with_compare_call_log(CONFLICTING_WITH_REFS, Some(PASS_JSON));
    assert_prompt_without_compare(&env, &log_path, "✗ Resolve conflict");
}

/// #410: `MERGE_STATE_UNKNOWN`（Calculating）は `behind_by` を捨てるため compare をスキップする。
#[test]
fn test_merge_state_unknown_skips_compare() {
    let (env, log_path) =
        branch_sync_fixtures::with_compare_call_log(MERGE_STATE_UNKNOWN_WITH_REFS, Some(PASS_JSON));
    assert_prompt_without_compare(&env, &log_path, "⧖ Wait for status");
}

/// #410: `UNKNOWN`（Calculating）も同様に compare をスキップする。
#[test]
fn test_unknown_status_skips_compare() {
    let (env, log_path) =
        branch_sync_fixtures::with_compare_call_log(UNKNOWN_WITH_REFS, Some(PASS_JSON));
    assert_prompt_without_compare(&env, &log_path, "⧖ Wait for status");
}
