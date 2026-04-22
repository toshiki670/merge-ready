//! ブランチ同期状態の E2E テスト（シナリオ #17–22）
//!
//! 対象条件: `conflict` / `update-branch` / `sync-unknown`
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

/// #17: `mergeable == CONFLICTING` → `✗ conflict`
#[test]
fn test_conflict() {
    let env = TestEnv::new(
        r#"{"state":"OPEN","isDraft":false,"mergeable":"CONFLICTING","mergeStateStatus":"DIRTY","reviewDecision":null}"#,
        Some(r#"[{"bucket":"pass","state":"SUCCESS"}]"#),
    );
    assert_prompt(&env, "✗ conflict");
}

/// #18: compare API の `behind_by > 0` → `✗ update-branch`
#[test]
fn test_update_branch() {
    let env = TestEnv::with_behind_by(
        r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"BLOCKED","reviewDecision":null,"baseRefName":"main","headRefName":"feat/test"}"#,
        Some(r#"[{"bucket":"pass","state":"SUCCESS"}]"#),
        1,
    );
    assert_prompt(&env, "✗ update-branch");
}

/// #19: compare API がエラーを返す → `? sync-unknown`
#[test]
fn test_compare_api_error() {
    let env = TestEnv::with_compare_error(
        r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"BLOCKED","reviewDecision":null,"baseRefName":"main","headRefName":"feat/test"}"#,
        Some(r#"[{"bucket":"pass","state":"SUCCESS"}]"#),
    );
    assert_prompt(&env, "? sync-unknown");
}

// ── 同期状態内の優先度 ────────────────────────────────────────────────────────

/// #20: `CONFLICTING` かつ `BEHIND` → `conflict` のみ表示（`update-branch` は抑制される）
#[test]
fn test_conflict_wins_over_update_branch() {
    let env = TestEnv::new(
        r#"{"state":"OPEN","isDraft":false,"mergeable":"CONFLICTING","mergeStateStatus":"BEHIND","reviewDecision":null}"#,
        Some(r#"[{"bucket":"pass","state":"SUCCESS"}]"#),
    );
    assert_prompt(&env, "✗ conflict");
}

// ── ci_checks との複合出力 ────────────────────────────────────────────────────

/// #21: `conflict` + `ci-fail` → 両方をスペース区切りで出力
#[test]
fn test_conflict_and_ci_fail() {
    let env = TestEnv::new(
        r#"{"state":"OPEN","isDraft":false,"mergeable":"CONFLICTING","mergeStateStatus":"DIRTY","reviewDecision":null}"#,
        Some(r#"[{"bucket":"fail","state":"FAILURE"}]"#),
    );
    assert_prompt(&env, "✗ conflict ✗ ci-fail");
}

// ── review との複合出力 ───────────────────────────────────────────────────────

/// #22: `conflict` + `review` → 両方をスペース区切りで出力
#[test]
fn test_conflict_and_review() {
    let env = TestEnv::new(
        r#"{"state":"OPEN","isDraft":false,"mergeable":"CONFLICTING","mergeStateStatus":"DIRTY","reviewDecision":"CHANGES_REQUESTED"}"#,
        Some(r#"[{"bucket":"pass","state":"SUCCESS"}]"#),
    );
    assert_prompt(&env, "✗ conflict ⚠ review");
}
