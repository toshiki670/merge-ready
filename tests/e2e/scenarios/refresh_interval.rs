//! リフレッシュ間隔 E2E テスト
//!
//! `#A`: Hot モード（`State::Calculating`）は短い間隔で gh を再フェッチする
//! `#B`: Warm モードは長い間隔を守り、テスト時間内に再フェッチしない

use super::super::helpers::DaemonHandle;
use super::refresh_interval_fixtures;

/// #A: `mergeStateStatus="UNKNOWN"` → Hot モード → 短い間隔で繰り返しリフレッシュされる
#[test]
fn test_hot_mode_refreshes_rapidly() {
    let (env, log_path) = refresh_interval_fixtures::with_calculating_pr_call_log();
    let _daemon = DaemonHandle::start_with_env(
        &env,
        &[
            ("MERGE_READY_HOT_WITH_QUERY_SECS", "1"),
            ("MERGE_READY_SCHEDULER_TICK_SECS", "1"),
            ("MERGE_READY_STALE_TTL", "0"),
        ],
    );

    DaemonHandle::wait_for_cache(&env, 5000);

    // Hot モードでは 1 秒ごとにリフレッシュされる。4 秒待って複数回呼ばれることを確認する。
    std::thread::sleep(std::time::Duration::from_secs(4));

    let call_log = std::fs::read_to_string(&log_path).unwrap_or_default();
    assert!(
        call_log.len() >= 3,
        "Hot mode should refresh at least 3 times in 4s, got {} calls",
        call_log.len()
    );
}

/// #B: Warm モードは `warm_refresh_secs` を守り、テスト時間内に再フェッチしない
#[test]
fn test_warm_mode_respects_longer_interval() {
    let (env, log_path) = refresh_interval_fixtures::with_warm_pr_call_log();
    let _daemon = DaemonHandle::start_with_env(
        &env,
        &[
            ("MERGE_READY_WARM_REFRESH_SECS", "60"),
            ("MERGE_READY_SCHEDULER_TICK_SECS", "1"),
            ("MERGE_READY_STALE_TTL", "0"),
        ],
    );

    DaemonHandle::wait_for_cache(&env, 5000);

    // Warm モードのリフレッシュ間隔は 60 秒。3 秒後も gh 呼び出しは初回分のみのはず。
    std::thread::sleep(std::time::Duration::from_secs(3));

    let call_log = std::fs::read_to_string(&log_path).unwrap_or_default();
    // pr list + pr checks + api compare = 最大 3 コマンド分が初回フェッチの呼び出し
    // リフレッシュが起きた場合はさらに同数増える
    let initial_calls = call_log.len();
    std::thread::sleep(std::time::Duration::from_secs(2));
    let later_calls = std::fs::read_to_string(&log_path).unwrap_or_default().len();
    assert_eq!(
        initial_calls, later_calls,
        "Warm mode should not refresh within warm_refresh_secs=60 (calls: {initial_calls} → {later_calls})"
    );
}
