//! `merge-ready` 判定の E2E テスト（シナリオ #15–16）
//!
//! `✓ merge-ready` が表示される条件と、全ブロッカーが同時成立したときに
//! `merge-ready` が表示されないことを検証する。
//! 実行フローは daemon 経由（`merge-ready prompt`）に統一する。

const PROMPT_BIN: &str = "merge-ready-prompt";

use assert_cmd::Command;
use predicates::prelude::*;

use super::super::helpers::{DaemonHandle, TestEnv};

/// daemon を起動してキャッシュを温め、`prompt` の出力を検証する。
fn assert_prompt(env: &TestEnv, expected: &str) {
    let _daemon = DaemonHandle::start(env);
    DaemonHandle::wait_for_cache(env, 5000);

    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert()
        .success()
        .stdout(predicate::str::diff(expected.to_owned()))
        .stderr("");
}

// ── #15: マージ可能 ──────────────────────────────────────────────────────────

/// #15: mergeable=MERGEABLE + CI pass + reviewDecision=APPROVED → `✓ merge-ready`
#[test]
fn test_merge_ready() {
    let env = TestEnv::new(
        r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":"APPROVED"}"#,
        Some(r#"[{"bucket":"pass","state":"SUCCESS"}]"#),
    );
    assert_prompt(&env, "✓ merge-ready");
}

// ── #16: 全ブロッカーが成立 ──────────────────────────────────────────────────

/// #16: conflict + ci-fail + review が全部成立 → `✓ merge-ready` は表示されない
#[test]
fn test_all_conditions_block_merge_ready() {
    let env = TestEnv::new(
        r#"{"state":"OPEN","isDraft":false,"mergeable":"CONFLICTING","mergeStateStatus":"DIRTY","reviewDecision":"CHANGES_REQUESTED"}"#,
        Some(r#"[{"bucket":"fail","state":"FAILURE"}]"#),
    );
    assert_prompt(&env, "✗ conflict ✗ ci-fail ⚠ review");
}
