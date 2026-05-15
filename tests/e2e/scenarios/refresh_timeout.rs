//! リフレッシュロックタイムアウト E2E テスト
//!
//! #I: gh が応答に時間がかかるとき、`refresh_lock_timeout_secs` を超えたら
//!     `clear_refresh_lock` でロックを解除し、スケジューラがリトライして最終的にデータを取得できること

use super::super::helpers::DaemonHandle;
use super::refresh_timeout_fixtures;

/// #I: 遅い gh → ロック期限切れ → `clear_refresh_lock` → リトライ → データ取得成功
#[test]
fn test_refresh_timeout_clears_lock_and_allows_retry() {
    let (env, log_path) = refresh_timeout_fixtures::with_slow_first_call();
    let _daemon = DaemonHandle::start_with_env(
        &env,
        &[
            ("MERGE_READY_REFRESH_LOCK_TIMEOUT_SECS", "1"),
            ("MERGE_READY_SCHEDULER_TICK_SECS", "1"),
            // stale_ttl=60 にして CacheEntry::new の fetched_at が過去分として大きな elapsed を持つようにする。
            // スケジューラのリトライ条件 fetched_at.elapsed() >= hot_with_query_secs を確実に満たすため。
            ("MERGE_READY_STALE_TTL", "60"),
            ("MERGE_READY_HOT_WITH_QUERY_SECS", "1"),
        ],
    );

    // 1 回目の gh は 5 秒かかる。lock_timeout=1s を超えると clear_refresh_lock が呼ばれ、
    // スケジューラが 2 回目を即時起動して成功する。
    // 最大 12 秒待つ（1s lock timeout + 1s retry + 5s slow gh の並走 + 余裕）
    DaemonHandle::wait_for_cache(&env, 12000);

    let calls = std::fs::read_to_string(&log_path).unwrap_or_default().len();
    assert!(
        calls >= 2,
        "should have at least 2 gh calls: initial (slow) + retry after lock timeout (calls: {calls})"
    );
}
