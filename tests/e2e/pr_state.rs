//! PR ライフサイクル状態の E2E テス���
//!
//! `OPEN` 以外の PR 状態、および PR が存在しない場合は何も出力しない。

use super::helpers::TestEnv;
use assert_cmd::Command;

fn cmd(env: &TestEnv) -> Command {
    let mut c = Command::cargo_bin("merge-ready").unwrap();
    env.apply(&mut c);
    c.arg("prompt");
    c
}

/// ブランチに PR が存在しない → 空文字（`exit 0`）
#[test]
fn test_no_pr() {
    let env = TestEnv::with_error(
        r#"no pull requests found for branch "feat/1-e2e-red-tests""#,
        1,
    );
    cmd(&env).assert().success().stdout("").stderr("");
}

/// PR が `CLOSED` → 空文字
#[test]
fn test_pr_closed() {
    let env = TestEnv::new(
        r#"{"state":"CLOSED","isDraft":false,"mergeable":"UNKNOWN","mergeStateStatus":"UNKNOWN","reviewDecision":null}"#,
        None,
    );
    cmd(&env).assert().success().stdout("").stderr("");
}

/// PR が `MERGED` → 空文字
#[test]
fn test_pr_merged() {
    let env = TestEnv::new(
        r#"{"state":"MERGED","isDraft":false,"mergeable":"UNKNOWN","mergeStateStatus":"UNKNOWN","reviewDecision":null}"#,
        None,
    );
    cmd(&env).assert().success().stdout("").stderr("");
}
