//! PR ライフサイクル状態の E2E テスト（シナリオ #31–33）
//!
//! `OPEN` 以外の PR 状態、および PR が存在しない場合は何も出力しない。
//! 実行フローは daemon 経由（`merge-ready prompt`）に統一する。

use assert_cmd::Command;
use rstest::rstest;

use super::super::helpers::{DaemonHandle, TestEnv};

const CLOSED_PR: &str = r#"{"state":"CLOSED","isDraft":false,"mergeable":"UNKNOWN","mergeStateStatus":"UNKNOWN","reviewDecision":null}"#;
const MERGED_PR: &str = r#"{"state":"MERGED","isDraft":false,"mergeable":"UNKNOWN","mergeStateStatus":"UNKNOWN","reviewDecision":null}"#;

fn assert_prompt_empty(env: &TestEnv) {
    let _daemon = DaemonHandle::start(env);
    DaemonHandle::wait_for_cache(env, 5000);

    let mut cmd = Command::cargo_bin("merge-ready-prompt").unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert().success().stdout("").stderr("");
}

// ── #31: PR なし ──────────────────────────────────────────────────────────────

/// #31: ブランチに PR が存在しない → 空文字（`exit 0`）
#[test]
fn test_no_pr() {
    let env = TestEnv::with_error(
        r#"no pull requests found for branch "feat/1-e2e-red-tests""#,
        1,
    );
    assert_prompt_empty(&env);
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
