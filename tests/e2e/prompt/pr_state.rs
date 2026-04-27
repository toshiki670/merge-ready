//! PR ライフサイクル状態の E2E テスト（シナリオ #31–33）
//!
//! `OPEN` 以外の PR 状態は何も出力しない。PR が存在しない場合は `+ create-pr` を出力する。
//! 実行フローは daemon 経由（`merge-ready prompt`）に統一する。

const PROMPT_BIN: &str = "merge-ready-prompt";

use assert_cmd::Command;
use predicates::prelude::*;
use rstest::rstest;

use super::super::helpers::{DaemonHandle, TestEnv};

const CLOSED_PR: &str = r#"{"state":"CLOSED","isDraft":false,"mergeable":"UNKNOWN","mergeStateStatus":"UNKNOWN","reviewDecision":null}"#;
const MERGED_PR: &str = r#"{"state":"MERGED","isDraft":false,"mergeable":"UNKNOWN","mergeStateStatus":"UNKNOWN","reviewDecision":null}"#;
const DRAFT_PR: &str = r#"{"state":"OPEN","isDraft":true,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null}"#;

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

fn assert_prompt_empty(env: &TestEnv) {
    let _daemon = DaemonHandle::start(env);
    DaemonHandle::wait_for_cache(env, 5000);

    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert().success().stdout("").stderr("");
}

// ── #31: PR なし ──────────────────────────────────────────────────────────────

/// #31: ブランチに PR が存在しない → `+ create-pr`（`exit 0`）
#[test]
fn test_no_pr() {
    let env = TestEnv::with_error(
        r#"no pull requests found for branch "feat/1-e2e-red-tests""#,
        1,
    );
    let _daemon = DaemonHandle::start(&env);
    DaemonHandle::wait_for_cache(&env, 5000);

    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert()
        .success()
        .stdout(predicate::str::diff("+ create-pr"))
        .stderr("");
}

// ── #32–33: CLOSED / MERGED ───────────────────────────────────────────────────

// ── #34: Draft PR ────────────────────────────────────────────────────────────

/// #34: Draft PR → `✎ ready-for-review`
#[test]
fn test_draft_pr_shows_ready_for_review() {
    let env = TestEnv::new(DRAFT_PR, Some(r#"[]"#));
    assert_prompt(&env, "✎ ready-for-review");
}

// ── #32–33: CLOSED / MERGED ───────────────────────────────────────────────────

/// #32 PR が `CLOSED` / #33 PR が `MERGED` → 空文字
#[rstest]
#[case::pr_closed(CLOSED_PR)]
#[case::pr_merged(MERGED_PR)]
fn test_non_open_pr_shows_nothing(#[case] pr_json: &str) {
    let env = TestEnv::new(pr_json, None);
    assert_prompt_empty(&env);
}
