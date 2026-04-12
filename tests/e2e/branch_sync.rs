//! ブランチ同期状態の E2E テスト
//!
//! 対象条件: `conflict` / `update-branch`
//! どちらもブランチとベースブランチの同期状態に起因する条件。
//! - 各条件の単体検証
//! - 同期状態内の優先度（`conflict` > `update-branch`）
//! - `ci_checks` / `review` との複合出力（`branch_sync` は独立して表示される）

use super::helpers::TestEnv;
use assert_cmd::Command;

fn cmd(env: &TestEnv) -> Command {
    let mut c = Command::cargo_bin("merge-ready").unwrap();
    env.apply(&mut c);
    c.args(["prompt", "--no-cache"]);
    c
}

// ─── 単体 ──────────────────────────────────────────────────────────────────

/// `mergeable == CONFLICTING` → `✗ conflict`
#[test]
fn test_conflict() {
    let env = TestEnv::new(
        r#"{"state":"OPEN","isDraft":false,"mergeable":"CONFLICTING","mergeStateStatus":"DIRTY","reviewDecision":null}"#,
        Some(r#"[{"bucket":"pass","state":"SUCCESS"}]"#),
    );
    cmd(&env).assert().success().stdout("✗ conflict").stderr("");
}

/// compare API の `behind_by > 0` → `✗ update-branch`（ブランチ保護設定に依存しない）
#[test]
fn test_update_branch() {
    let env = TestEnv::with_behind_by(
        r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"BLOCKED","reviewDecision":null,"baseRefName":"main","headRefName":"feat/test"}"#,
        Some(r#"[{"bucket":"pass","state":"SUCCESS"}]"#),
        1,
    );
    cmd(&env)
        .assert()
        .success()
        .stdout("✗ update-branch")
        .stderr("");
}

/// compare API が失敗した場合 → `? sync-unknown`
#[test]
fn test_compare_api_error() {
    let env = TestEnv::with_compare_error(
        r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"BLOCKED","reviewDecision":null,"baseRefName":"main","headRefName":"feat/test"}"#,
        Some(r#"[{"bucket":"pass","state":"SUCCESS"}]"#),
    );
    cmd(&env)
        .assert()
        .success()
        .stdout("? sync-unknown")
        .stderr("");
}

// ─── 同期状態内の優先度 ───────────────────────────────────────────────────

/// `CONFLICTING` かつ `BEHIND` → `conflict` のみ表示（`update-branch` は抑制される）
#[test]
fn test_conflict_wins_over_update_branch() {
    let env = TestEnv::new(
        r#"{"state":"OPEN","isDraft":false,"mergeable":"CONFLICTING","mergeStateStatus":"BEHIND","reviewDecision":null}"#,
        Some(r#"[{"bucket":"pass","state":"SUCCESS"}]"#),
    );
    cmd(&env).assert().success().stdout("✗ conflict").stderr("");
}

// ─── ci_checks との複合出力 ───────────────────────────────────────────────

/// `conflict` + `ci-fail` → 両方をスペース区切りで出力（`branch_sync` は `ci_checks` を抑制しない）
#[test]
fn test_conflict_and_ci_fail() {
    let env = TestEnv::new(
        r#"{"state":"OPEN","isDraft":false,"mergeable":"CONFLICTING","mergeStateStatus":"DIRTY","reviewDecision":null}"#,
        Some(r#"[{"bucket":"fail","state":"FAILURE"}]"#),
    );
    cmd(&env)
        .assert()
        .success()
        .stdout("✗ conflict ✗ ci-fail")
        .stderr("");
}

// ─── review との複合出力 ──────────────────────────────────────────────────

/// `conflict` + `review` → 両方をスペース区切りで出力
#[test]
fn test_conflict_and_review() {
    let env = TestEnv::new(
        r#"{"state":"OPEN","isDraft":false,"mergeable":"CONFLICTING","mergeStateStatus":"DIRTY","reviewDecision":"CHANGES_REQUESTED"}"#,
        Some(r#"[{"bucket":"pass","state":"SUCCESS"}]"#),
    );
    cmd(&env)
        .assert()
        .success()
        .stdout("✗ conflict ⚠ review")
        .stderr("");
}
