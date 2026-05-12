//! Warm → Cold モード遷移シナリオ
//!
//! `MERGE_READY_WARM_TO_COLD_SECS=0` で Query がなくなると即 Cold に移行する。
//! Cold モードでも `COLD_EARLY_SECS=0` / `COLD_LATE_SECS=0` によりスケジューラ tick ごとに
//! リフレッシュが発生することを gh 呼び出し回数で検証する。
//!
//! カバー:
//! - `CacheEntry::increment_cold_count`
//! - `CacheEntry::is_cold`
//! - `CacheEntry::cold_refresh_count` (read in `cold_interval_secs`)
//! - `RefreshPolicy::effective_refresh_interval_secs` の Warm + is_cold 分岐
//! - `RefreshPolicy::cold_interval_secs` の Cold late 分岐

const PROMPT_BIN: &str = "merge-ready-prompt";

use assert_cmd::cargo::cargo_bin;

use super::super::helpers::{DaemonHandle, run_prompt_with_timeout};
use super::cold_mode_fixtures;

/// Query がなくても Cold モードでリフレッシュが継続する
#[test]
fn test_cold_mode_continues_refreshing_without_queries() {
    let (env, log_path) = cold_mode_fixtures::with_open_pr_call_log();
    let _daemon = DaemonHandle::start_with_env(
        &env,
        &[
            ("MERGE_READY_WARM_TO_COLD_SECS", "0"),
            ("MERGE_READY_COLD_EARLY_SECS", "0"),
            ("MERGE_READY_COLD_EARLY_LIMIT", "1"),
            ("MERGE_READY_COLD_LATE_SECS", "0"),
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

    // キャッシュ温まるまで待つ
    DaemonHandle::wait_for_cache(&env, 5000);

    // Query を送らずに 3 秒待機（warm_to_cold_secs=0 で即 Cold → cold_early/late_secs=0 で毎 tick）
    std::thread::sleep(std::time::Duration::from_millis(3000));

    // gh が 2 回以上呼ばれていること（Cold モードでも自動リフレッシュが継続）
    let call_log = std::fs::read_to_string(&log_path).unwrap_or_default();
    assert!(
        call_log.len() > 1,
        "Cold モードで gh が 2 回以上呼ばれるはず（実際: {} 回）",
        call_log.len()
    );
}
