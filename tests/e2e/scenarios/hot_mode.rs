//! Hot モード高速リフレッシュシナリオ
//!
//! CI 実行中（pending）の PR → `RefreshMode::Hot` → スケジューラ tick ごとに再フェッチ。
//! `HOT_WITH_QUERY_SECS=0` により直近 Query あり Hot エントリは毎 tick リフレッシュされる。
//!
//! カバー:
//! - `RefreshPolicy::effective_refresh_interval_secs` の Hot 分岐
//! - `CacheEntry::has_recent_query` (true パス)
//! - `background_refresh::collect_targets` の interval チェック到達

const PROMPT_BIN: &str = "merge-ready-prompt";

use assert_cmd::cargo::cargo_bin;

use super::super::helpers::{DaemonHandle, run_prompt_with_timeout};
use super::hot_mode_fixtures;

/// CI pending の Hot モードエントリがスケジューラ tick で複数回リフレッシュされる
#[test]
fn test_hot_mode_refreshes_multiple_times() {
    let (env, log_path) = hot_mode_fixtures::with_ci_pending_call_log();
    let _daemon = DaemonHandle::start_with_env(
        &env,
        &[
            ("MERGE_READY_HOT_WITH_QUERY_SECS", "0"),
            ("MERGE_READY_HOT_WITHOUT_QUERY_SECS", "0"),
            ("MERGE_READY_STALE_TTL", "0"),
            ("MERGE_READY_SCHEDULER_TICK_SECS", "1"),
        ],
    );

    let bin = cargo_bin(PROMPT_BIN);

    // 初回クエリ
    run_prompt_with_timeout(
        std::process::Command::new(&bin)
            .env("PATH", env.path_env())
            .env("HOME", env.home())
            .env("TMPDIR", env.home())
            .current_dir(env.repo.path()),
    );

    // キャッシュ温まるまで待つ（CI pending 出力）
    DaemonHandle::wait_for_cache(&env, 5000);

    // has_recent_query を true に保つためクエリを繰り返す
    for _ in 0..3 {
        run_prompt_with_timeout(
            std::process::Command::new(&bin)
                .env("PATH", env.path_env())
                .env("HOME", env.home())
                .env("TMPDIR", env.home())
                .current_dir(env.repo.path()),
        );
    }

    // スケジューラ 2 tick 分（2 秒）待機
    std::thread::sleep(std::time::Duration::from_millis(2000));

    // gh が 2 回以上呼ばれていること（Hot モードで再フェッチが発生）
    let call_log = std::fs::read_to_string(&log_path).unwrap_or_default();
    assert!(
        call_log.len() > 1,
        "Hot モードで gh が 2 回以上呼ばれるはず（実際: {} 回）",
        call_log.len()
    );
}
