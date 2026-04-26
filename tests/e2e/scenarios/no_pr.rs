//! キャッシュライフサイクル シナリオ #4, #5: PR なし
//!
//! #4: PR なしブランチ → loading → キャッシュ確定後に空出力（`? loading` が永続しない）
//! #5: PR なし + TTL=0 + リフレッシュ遅延 → stale 中も `? loading` に戻らず空出力維持

const PROMPT_BIN: &str = "merge-ready-prompt";

use assert_cmd::Command;
use predicates::prelude::*;

use super::super::helpers::{DaemonHandle, TestEnv};

/// #4: PR なしブランチで daemon 経由: リフレッシュ完了後に `? loading` が消える
#[test]
fn test_daemon_no_pr_shows_nothing_after_refresh() {
    let env = TestEnv::with_no_pr();
    let _daemon = DaemonHandle::start(&env);

    // 初回クエリ: キャッシュミス → ? loading
    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert()
        .success()
        .stdout(predicate::str::diff("? loading"));

    // daemon リフレッシュ完了を待つ
    DaemonHandle::wait_for_cache(&env, 5000);

    // キャッシュ確定後は何も出力しない（? loading が永続しないことを確認）
    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert().success().stdout(predicate::str::is_empty());
}

/// #5: no-PR の stale リフレッシュ中でも `? loading` に戻らず空出力を維持する
#[test]
fn test_daemon_no_pr_stale_while_refreshing_keeps_empty_output() {
    let env = TestEnv::with_no_pr_stale_delay_ms(1000);
    let _daemon = DaemonHandle::start_with_env(&env, &[("MERGE_READY_STALE_TTL", "0")]);

    // 初回クエリ: キャッシュミス → loading
    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert()
        .success()
        .stdout(predicate::str::diff("? loading"));

    // 初回リフレッシュ完了で空キャッシュ確定
    DaemonHandle::wait_for_cache(&env, 5000);

    // stale 1回目: リフレッシュ開始しつつ空出力
    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert().success().stdout(predicate::str::is_empty());

    // stale 2回目以降（refresh 実行中を狙う）: loading に戻らず空出力を維持
    for _ in 0..5 {
        let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
        env.apply_with_cache(&mut cmd);
        cmd.assert().success().stdout(predicate::str::is_empty());
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}
