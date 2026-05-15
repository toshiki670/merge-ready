//! Warm モード間隔バリアント E2E テスト
//!
//! #F: Warm + recent query → `hot_with_query_secs` が適用される（高頻度リフレッシュ）
//! #G: Warm + no recent query + not cold → `warm_refresh_secs` が適用される（中間状態）

use super::super::helpers::DaemonHandle;
use super::warm_interval_fixtures;

/// #F: Warm PR でも recent query があれば `hot_with_query_secs` で高頻度リフレッシュされる
#[test]
fn test_warm_with_recent_query_uses_hot_interval() {
    let (env, log_path) = warm_interval_fixtures::with_warm_pr_call_log();
    let _daemon = DaemonHandle::start_with_env(
        &env,
        &[
            ("MERGE_READY_HOT_RECENT_QUERY_SECS", "60"),
            ("MERGE_READY_HOT_WITH_QUERY_SECS", "1"),
            ("MERGE_READY_WARM_REFRESH_SECS", "60"),
            ("MERGE_READY_STALE_TTL", "0"),
            ("MERGE_READY_SCHEDULER_TICK_SECS", "1"),
        ],
    );

    // wait_for_cache は内部でクエリを送り続けるので last_queried_at が更新される
    DaemonHandle::wait_for_cache(&env, 5000);

    // has_recent_query(60s)=true の状態で 2 秒待つ
    // hot_with_query_secs=1 なので 2 回以上リフレッシュされる
    std::thread::sleep(std::time::Duration::from_secs(2));

    let calls = std::fs::read_to_string(&log_path).unwrap_or_default().len();
    assert!(
        calls >= 2,
        "Warm PR with recent query should refresh at hot_with_query_secs=1 \
         (at least 2 calls in 2s, got {calls})"
    );
}

/// #G: Warm PR で recent query がなく Cold でもない中間状態では `warm_refresh_secs` が適用される
#[test]
fn test_warm_without_recent_query_uses_warm_interval() {
    let (env, log_path) = warm_interval_fixtures::with_warm_pr_call_log();
    let _daemon = DaemonHandle::start_with_env(
        &env,
        &[
            ("MERGE_READY_HOT_RECENT_QUERY_SECS", "1"),
            // hot_with_query_secs を大きくして「recent query あり」期間中のリフレッシュを防ぐ。
            // これにより warm_refresh_secs=4 のパスのみが有効になる。
            ("MERGE_READY_HOT_WITH_QUERY_SECS", "60"),
            ("MERGE_READY_WARM_REFRESH_SECS", "4"),
            ("MERGE_READY_WARM_TO_COLD_SECS", "60"),
            ("MERGE_READY_STALE_TTL", "60"),
            ("MERGE_READY_SCHEDULER_TICK_SECS", "1"),
        ],
    );

    DaemonHandle::wait_for_cache(&env, 5000);
    let initial_calls = std::fs::read_to_string(&log_path).unwrap_or_default().len();

    // 2 秒待つ: hot_recent_query_secs=1 を超えた → no recent query
    // ただし warm_to_cold_secs=60 未満 → cold でもない（中間状態）
    // warm_refresh_secs=4 なので fetched_at.elapsed() < 4 → まだリフレッシュしない
    // hot_with_query_secs=60 なので recent 期間中も早期リフレッシュは起きない
    std::thread::sleep(std::time::Duration::from_secs(2));

    let calls_at_2s = std::fs::read_to_string(&log_path).unwrap_or_default().len();

    // さらに 4 秒待つ: 合計 6 秒 → fetched_at.elapsed() >= warm_refresh_secs=4 → リフレッシュ
    std::thread::sleep(std::time::Duration::from_secs(4));

    let calls_at_6s = std::fs::read_to_string(&log_path).unwrap_or_default().len();

    assert_eq!(
        calls_at_2s, initial_calls,
        "within warm_refresh_secs=4, no extra refresh should happen \
         (initial: {initial_calls}, at 2s: {calls_at_2s})"
    );
    assert!(
        calls_at_6s > initial_calls,
        "after warm_refresh_secs=4, a background refresh should have occurred \
         (initial: {initial_calls}, at 6s: {calls_at_6s})"
    );
}
