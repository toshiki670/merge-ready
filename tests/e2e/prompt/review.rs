//! レビュー状態の E2E テスト（シナリオ #30, #31）
//!
//! 対象条件:
//!   #30: `reviewDecision == CHANGES_REQUESTED` → `⚠ Resolve review`
//!   #31: `reviewDecision == REVIEW_REQUIRED`   → `@ Assign reviewer`
//! 実行フローは daemon 経由（`merge-ready prompt`）に統一する。

const PROMPT_BIN: &str = "merge-ready-prompt";

use assert_cmd::Command;

use super::super::helpers::{DaemonHandle, TestEnv};

/// #30: `reviewDecision == CHANGES_REQUESTED` → `⚠ Resolve review`
#[test]
fn test_review_changes_requested() {
    let env = TestEnv::new(
        r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"BLOCKED","reviewDecision":"CHANGES_REQUESTED"}"#,
        Some(r#"[{"bucket":"pass","state":"SUCCESS"}]"#),
    );
    let _daemon = DaemonHandle::start(&env);
    DaemonHandle::wait_for_cache(&env, 5000);

    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert().success().stdout("⚠ Resolve review").stderr("");
}

/// #31: `reviewDecision == REVIEW_REQUIRED` → `@ Assign reviewer`
#[test]
fn test_review_required() {
    let env = TestEnv::new(
        r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"BLOCKED","reviewDecision":"REVIEW_REQUIRED"}"#,
        Some(r#"[{"bucket":"pass","state":"SUCCESS"}]"#),
    );
    let _daemon = DaemonHandle::start(&env);
    DaemonHandle::wait_for_cache(&env, 5000);

    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert()
        .success()
        .stdout("@ Assign reviewer")
        .stderr("");
}
