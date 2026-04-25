//! キャッシュライフサイクル シナリオ #3: stale
//!
//! TTL 超過 → stale 値を返す + 裏でリフレッシュ → 次回は新鮮な値

use assert_cmd::Command;
use predicates::prelude::*;

use super::super::helpers::{DaemonHandle, TestEnv};

const OPEN_PR_VIEW_JSON: &str = r#"{"state":"OPEN","isDraft":false,"mergeable":"MERGEABLE","mergeStateStatus":"CLEAN","reviewDecision":null}"#;
const CI_PASS_JSON: &str = r#"[{"bucket":"pass","state":"SUCCESS","name":"ci","link":""}]"#;

/// TTL=0 で起動した daemon → stale でも値を返す（`? loading` に戻らない）
#[test]
fn test_daemon_stale_returns_output() {
    let env = TestEnv::new(OPEN_PR_VIEW_JSON, Some(CI_PASS_JSON));
    let _daemon = DaemonHandle::start_with_env(&env, &[("MERGE_READY_STALE_TTL", "0")]);

    // 初回: キャッシュミス → ? loading
    let mut cmd = Command::cargo_bin("merge-ready-prompt").unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert()
        .success()
        .stdout(predicate::str::diff("? loading"));

    // キャッシュが温まるまで待つ
    DaemonHandle::wait_for_cache(&env, 5000);

    // TTL=0 なので stale だが、それでも値を返すこと（`? loading` に戻らない）
    let mut cmd = Command::cargo_bin("merge-ready-prompt").unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert().success().stdout(predicate::str::contains("✓"));
}
