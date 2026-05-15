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
    DaemonHandle::wait_for_cache(&env, 15000);

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
            ("MERGE_READY_HOT_RECENT_QUERY_SECS", "0"),
            // warm_refresh_secs=1 のパスを有効にする。cold に遷移しないよう warm_to_cold=60。
            ("MERGE_READY_HOT_WITH_QUERY_SECS", "60"),
            ("MERGE_READY_WARM_REFRESH_SECS", "1"),
            ("MERGE_READY_WARM_TO_COLD_SECS", "60"),
            ("MERGE_READY_STALE_TTL", "60"),
            ("MERGE_READY_SCHEDULER_TICK_SECS", "1"),
        ],
    );

    DaemonHandle::wait_for_cache(&env, 15000);
    let initial_calls = std::fs::read_to_string(&log_path).unwrap_or_default().len();

    // hot_recent_query_secs=0 なので wait_for_cache 直後 ~1s で has_recent_query=false になり
    // warm_refresh_secs=1 が適用される。3s 待てばスケジューラが複数回リフレッシュを実施する。
    std::thread::sleep(std::time::Duration::from_secs(3));

    let calls_after = std::fs::read_to_string(&log_path).unwrap_or_default().len();
    assert!(
        calls_after > initial_calls,
        "warm_refresh_secs=1 should trigger at least one background refresh within 3s \
         (initial: {initial_calls}, after: {calls_after})"
    );
}
