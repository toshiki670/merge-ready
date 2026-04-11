//! `review` 条件の E2E テスト
//!
//! 対象条件: `reviewDecision == CHANGES_REQUESTED` → `⚠ review`
//! `branch_sync` / `ci_checks` とは独立したグループ外条件。
//! 複合ケースは各グループのテストファイルに配置（co-locate）。

use super::helpers::TestEnv;
use assert_cmd::Command;

/// `reviewDecision == CHANGES_REQUESTED` → `⚠ review`
#[test]
fn test_review_changes_requested() {
    let env = TestEnv::new(
        r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"BLOCKED","reviewDecision":"CHANGES_REQUESTED"}"#,
        Some(r#"[{"bucket":"pass","state":"SUCCESS"}]"#),
    );
    let mut cmd = Command::cargo_bin("merge-ready").unwrap();
    env.apply(&mut cmd);
    cmd.assert().success().stdout("⚠ review").stderr("");
}
