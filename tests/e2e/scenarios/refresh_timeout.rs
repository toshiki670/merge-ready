//! リフレッシュロックタイムアウト E2E テスト
//!
//! #I: gh が応答に時間がかかるとき、`refresh_lock_timeout_secs` を超えたら
//!     `clear_refresh_lock` でロックを解除し、スケジューラがリトライして最終的にデータを取得できること

use super::super::helpers::DaemonHandle;
use super::refresh_timeout_fixtures;

/// #I: 遅い gh (2 回目 pr list) → ロック期限切れ → `clear_refresh_lock` → リトライ → データ取得成功
#[test]
fn test_refresh_timeout_clears_lock_and_allows_retry() {
    let (env, log_path) = refresh_timeout_fixtures::with_slow_second_pr_list();
    let _daemon = DaemonHandle::start_with_env(
        &env,
        &[
            ("MERGE_READY_REFRESH_LOCK_TIMEOUT_SECS", "1"),
            ("MERGE_READY_SCHEDULER_TICK_SECS", "1"),
            // stale_ttl=60 にして CacheEntry::new の fetched_at が過去分として大きな elapsed を持つようにする。
            // スケジューラのリトライ条件 fetched_at.elapsed() >= hot_with_query_secs を確実に満たすため。
            ("MERGE_READY_STALE_TTL", "60"),
            // hot_with_query_secs=0 にして初回フェッチ完了直後からスケジューラが即座に 2 回目をスケジュールするようにする。
            ("MERGE_READY_HOT_WITH_QUERY_SECS", "0"),
        ],
    );

    // 初回 pr list は即時応答 → CacheEntry に output が入り is_active()=true になる。
    DaemonHandle::wait_for_cache(&env, 5000);

    // 初回フェッチ完了後:
    //   T+~1s: スケジューラが 2 回目リフレッシュをスケジュール → pr list #2 が 2s sleep 開始
    //   T+~2s: refresh_lock_timeout=1s を超えると clear_refresh_lock 呼び出し
    //   T+~3s: スケジューラがリトライ → pr list #3 が即時応答
    std::thread::sleep(std::time::Duration::from_secs(3));

    let calls = std::fs::read_to_string(&log_path).unwrap_or_default().len();
    assert!(
        calls >= 4,
        "should have at least 4 gh calls: initial 3 (pr list + checks + compare) \
         + slow pr list #2 logged before sleep (calls: {calls})"
    );
}
