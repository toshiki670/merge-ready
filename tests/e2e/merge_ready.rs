//! `merge-ready` 判定の E2E テスト
//!
//! `✓ merge-ready` が表示される条件（全ブロッカーが存在しない）と、
//! 全ブロッカーが同時成立したときに `merge-ready` が表示されないことを検証する。

use super::helpers::TestEnv;
use assert_cmd::Command;

fn cmd(env: &TestEnv) -> Command {
    let mut c = Command::cargo_bin("merge-ready").unwrap();
    env.apply(&mut c);
    c
}

/// `conflict` なし + CI pass + `review` 承認済み → `✓ merge-ready`
#[test]
fn test_merge_ready() {
    let env = TestEnv::new(
        r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":"APPROVED"}"#,
        Some(r#"[{"bucket":"pass","state":"SUCCESS"}]"#),
    );
    cmd(&env)
        .assert()
        .success()
        .stdout("✓ merge-ready")
        .stderr("");
}

/// `conflict` + `ci-fail` + `review` が全部成立 → `✓ merge-ready` は表示されない
#[test]
fn test_all_conditions_block_merge_ready() {
    let env = TestEnv::new(
        r#"{"state":"OPEN","isDraft":false,"mergeable":"CONFLICTING","mergeStateStatus":"DIRTY","reviewDecision":"CHANGES_REQUESTED"}"#,
        Some(r#"[{"bucket":"fail","state":"FAILURE"}]"#),
    );
    cmd(&env)
        .assert()
        .success()
        .stdout("✗ conflict ✗ ci-fail ⚠ review")
        .stderr("");
}
