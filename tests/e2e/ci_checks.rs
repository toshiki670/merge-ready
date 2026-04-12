//! CI チェックの E2E テスト
//!
//! 対象条件: `ci-fail` / `ci-action`（`gh pr checks --json bucket,state` の結果）
//! - 各条件の単体検証（`fail` / `cancel` どちらも `ci-fail`）
//! - `ci_checks` 内の優先度（`ci-fail` > `ci-action`）
//! - `review` との複合出力（`ci_checks` は `review` を抑制しない）

use super::helpers::TestEnv;
use assert_cmd::Command;

fn cmd(env: &TestEnv) -> Command {
    let mut c = Command::cargo_bin("merge-ready").unwrap();
    env.apply(&mut c);
    c.arg("prompt");
    c
}

// ─── 単体 ──────────────────────────────────────────────────────────────────

/// `checks bucket == fail` → `✗ ci-fail`
#[test]
fn test_ci_fail_failure() {
    let env = TestEnv::new(
        r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"BLOCKED","reviewDecision":null}"#,
        Some(r#"[{"bucket":"fail","state":"FAILURE"}]"#),
    );
    cmd(&env).assert().success().stdout("✗ ci-fail").stderr("");
}

/// `checks bucket == cancel` も `✗ ci-fail` として扱う
#[test]
fn test_ci_fail_cancelled() {
    let env = TestEnv::new(
        r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"BLOCKED","reviewDecision":null}"#,
        Some(r#"[{"bucket":"cancel","state":"CANCELLED"}]"#),
    );
    cmd(&env).assert().success().stdout("✗ ci-fail").stderr("");
}

/// `checks bucket == action_required` → `⚠ ci-action`
#[test]
fn test_ci_action() {
    let env = TestEnv::new(
        r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"BLOCKED","reviewDecision":null}"#,
        Some(r#"[{"bucket":"action_required","state":"ACTION_REQUIRED"}]"#),
    );
    cmd(&env)
        .assert()
        .success()
        .stdout("⚠ ci-action")
        .stderr("");
}

// ─── ci_checks 内の優先度 ─────────────────────────────────────────────────

/// `fail` と `action_required` が混在 → `ci-fail` のみ表示（`ci-action` は抑制される）
#[test]
fn test_ci_fail_wins_over_ci_action() {
    let env = TestEnv::new(
        r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"BLOCKED","reviewDecision":null}"#,
        Some(
            r#"[{"bucket":"fail","state":"FAILURE"},{"bucket":"action_required","state":"ACTION_REQUIRED"}]"#,
        ),
    );
    cmd(&env).assert().success().stdout("✗ ci-fail").stderr("");
}

// ─── CI 未設定 ──────────────────────────────────────────────────────────────

/// `gh pr checks` が "no checks reported" で失敗 → 空リスト扱い → `✓ merge-ready`
#[test]
fn test_no_ci_checks_merge_ready() {
    let env = TestEnv::with_no_ci_checks(
        r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":"APPROVED"}"#,
    );
    cmd(&env)
        .assert()
        .success()
        .stdout("✓ merge-ready")
        .stderr("");
}

/// `gh pr checks` が "no checks reported" + `review` あり → CI なし扱いで `⚠ review` のみ出力
#[test]
fn test_no_ci_checks_with_review() {
    let env = TestEnv::with_no_ci_checks(
        r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"BLOCKED","reviewDecision":"CHANGES_REQUESTED"}"#,
    );
    cmd(&env).assert().success().stdout("⚠ review").stderr("");
}

// ─── review との複合出力 ──────────────────────────────────────────────────

/// `ci-fail` + `review` → 両方をスペース区切りで出力（`ci_checks` は `review` を抑制しない）
#[test]
fn test_ci_fail_and_review() {
    let env = TestEnv::new(
        r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"BLOCKED","reviewDecision":"CHANGES_REQUESTED"}"#,
        Some(r#"[{"bucket":"fail","state":"FAILURE"}]"#),
    );
    cmd(&env)
        .assert()
        .success()
        .stdout("✗ ci-fail ⚠ review")
        .stderr("");
}
