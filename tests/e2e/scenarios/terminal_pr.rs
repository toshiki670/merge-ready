//! terminal PR シナリオ: PR が closed / merged になったらリフレッシュを停止する（シナリオ #50, #51）
//!
//! #50: closed PR → キャッシュ確定後に TTL=0 で連続クエリ → gh は1回しか呼ばれない
//! #51: merged PR → 同上

const PROMPT_BIN: &str = "merge-ready-prompt";

use assert_cmd::Command;
use predicates::prelude::*;

use super::super::helpers::{DaemonHandle, TestEnv};

/// #50: closed PR はキャッシュ確定後に再フェッチされない
#[test]
fn test_closed_pr_stops_refreshing_after_terminal_state() {
    let (env, log_path) = TestEnv::with_terminal_pr_call_log("CLOSED");
    let _daemon = DaemonHandle::start_with_env(&env, &[("MERGE_READY_STALE_TTL", "0")]);

    // 初回クエリ: キャッシュミス → ? loading
    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert()
        .success()
        .stdout(predicate::str::diff("? loading"));

    // キャッシュ確定（closed PR → output=""、is_terminal=true）を待つ
    DaemonHandle::wait_for_cache(&env, 5000);

    // TTL=0 でも terminal エントリは background_refresh_secs TTL を使うため
    // 連続クエリで gh が再度呼ばれないことを確認
    for _ in 0..5 {
        let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
        env.apply_with_cache(&mut cmd);
        cmd.assert().success().stdout(predicate::str::is_empty());
    }

    // gh 呼び出し回数 = 1（初回リフレッシュのみ）
    let call_log = std::fs::read_to_string(&log_path).unwrap_or_default();
    assert_eq!(
        call_log.len(),
        1,
        "terminal closed PR は初回リフレッシュ後に gh を呼ばないはず（呼び出し回数: {}）",
        call_log.len()
    );
}

/// #51: merged PR はキャッシュ確定後に再フェッチされない
#[test]
fn test_merged_pr_stops_refreshing_after_terminal_state() {
    let (env, log_path) = TestEnv::with_terminal_pr_call_log("MERGED");
    let _daemon = DaemonHandle::start_with_env(&env, &[("MERGE_READY_STALE_TTL", "0")]);

    // 初回クエリ: ? loading
    let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
    env.apply_with_cache(&mut cmd);
    cmd.assert()
        .success()
        .stdout(predicate::str::diff("? loading"));

    DaemonHandle::wait_for_cache(&env, 5000);

    for _ in 0..5 {
        let mut cmd = Command::cargo_bin(PROMPT_BIN).unwrap();
        env.apply_with_cache(&mut cmd);
        cmd.assert().success().stdout(predicate::str::is_empty());
    }

    let call_log = std::fs::read_to_string(&log_path).unwrap_or_default();
    assert_eq!(
        call_log.len(),
        1,
        "terminal merged PR は初回リフレッシュ後に gh を呼ばないはず（呼び出し回数: {}）",
        call_log.len()
    );
}
