//! Cold モード E2E テスト
//!
//! #D1: クエリが途絶えると Cold 遷移し、スケジューラが cold リフレッシュする（`increment_cold_count`）
//! #D2: `cold_early_limit` 到達後に `cold_late_secs` へ間隔が切り替わる（`cold_interval_secs` early/late）
//! #D3: Cold 中にクエリが来ると cold カウンタがリセットされ early 間隔に戻る（`reset_cold_count`）

use super::super::helpers::DaemonHandle;
use super::cold_mode_fixtures;

/// #D1: クエリが途絶えると Cold 遷移し、スケジューラが bg リフレッシュを実行する
#[test]
fn test_cold_refresh_increments_count() {
    let (env, log_path) = cold_mode_fixtures::with_warm_pr_call_log();
    let _daemon = DaemonHandle::start_with_env(
        &env,
        &[
            ("MERGE_READY_WARM_TO_COLD_SECS", "1"),
            ("MERGE_READY_COLD_EARLY_SECS", "1"),
            ("MERGE_READY_WARM_REFRESH_SECS", "60"),
            ("MERGE_READY_HOT_RECENT_QUERY_SECS", "1"),
            ("MERGE_READY_STALE_TTL", "60"),
            ("MERGE_READY_SCHEDULER_TICK_SECS", "1"),
        ],
    );

    DaemonHandle::wait_for_cache(&env, 5000);

    // warm_to_cold_secs=1 を超えて Cold 遷移し、cold_early_secs=1 で bg リフレッシュ発生
    std::thread::sleep(std::time::Duration::from_secs(3));

    let calls = std::fs::read_to_string(&log_path).unwrap_or_default().len();
    assert!(
        calls >= 2,
        "cold mode should trigger at least 1 bg refresh after going cold (calls: {calls})"
    );
}

/// #D2: `cold_early_limit` 回の cold リフレッシュ後、間隔が `cold_late_secs` に切り替わる
#[test]
fn test_cold_interval_switches_from_early_to_late() {
    let (env, log_path) = cold_mode_fixtures::with_warm_pr_call_log();
    let _daemon = DaemonHandle::start_with_env(
        &env,
        &[
            // warm_to_cold_secs=0 → 常に cold。warm → cold 遷移待ちを省いて cold_early を素早く発火させる。
            ("MERGE_READY_WARM_TO_COLD_SECS", "0"),
            ("MERGE_READY_COLD_EARLY_SECS", "1"),
            ("MERGE_READY_COLD_LATE_SECS", "60"),
            ("MERGE_READY_COLD_EARLY_LIMIT", "1"),
            ("MERGE_READY_WARM_REFRESH_SECS", "60"),
            ("MERGE_READY_HOT_RECENT_QUERY_SECS", "0"),
            ("MERGE_READY_STALE_TTL", "60"),
            ("MERGE_READY_SCHEDULER_TICK_SECS", "1"),
        ],
    );

    DaemonHandle::wait_for_cache(&env, 5000);

    // warm_to_cold_secs=0 なので即 cold。cold_early_secs=1 で 1 回 cold refresh (count → 1)
    // count=1 >= cold_early_limit=1 → 次の間隔は cold_late_secs=60
    // hot_recent_query_secs=0 なので wait_for_cache 直後も has_recent_query=false
    std::thread::sleep(std::time::Duration::from_secs(2));

    let calls_after_early = std::fs::read_to_string(&log_path).unwrap_or_default().len();
    assert!(
        calls_after_early >= 3,
        "should have at least 1 cold-early refresh after initial fetch (calls: {calls_after_early})"
    );

    // cold_early_limit=1 に到達 → 次は cold_late_secs=60 → この 1 秒では追加リフレッシュなし
    std::thread::sleep(std::time::Duration::from_secs(1));

    let calls_after_late = std::fs::read_to_string(&log_path).unwrap_or_default().len();
    assert_eq!(
        calls_after_early, calls_after_late,
        "after reaching cold_early_limit, cold_late_secs=60 should prevent further refreshes \
         (calls before: {calls_after_early}, after: {calls_after_late})"
    );
}

/// #D3: Cold 中にクエリが来ると cold カウンタがリセットされ early 間隔に戻る
#[test]
fn test_cold_reset_on_warm_query() {
    let (env, log_path) = cold_mode_fixtures::with_warm_pr_call_log();
    let _daemon = DaemonHandle::start_with_env(
        &env,
        &[
            // warm_to_cold_secs=0 → 常に cold。is_cold_or_never_queried が必ず true になる。
            ("MERGE_READY_WARM_TO_COLD_SECS", "0"),
            ("MERGE_READY_COLD_EARLY_SECS", "1"),
            ("MERGE_READY_COLD_LATE_SECS", "60"),
            ("MERGE_READY_COLD_EARLY_LIMIT", "1"),
            // recent query でも間隔が長いため、リセット後の early 間隔適用をノイズなく確認できる
            ("MERGE_READY_HOT_WITH_QUERY_SECS", "60"),
            ("MERGE_READY_WARM_REFRESH_SECS", "60"),
            ("MERGE_READY_HOT_RECENT_QUERY_SECS", "0"),
            // stale_ttl=1 にして cold カウンタリセット用クエリが Stale パスを通るようにする。
            // reset_cold_count は process_query の Stale パスで is_cold_or_never_queried=true のときのみ呼ばれる。
            // is_fresh(1) は fetched_at.elapsed().as_secs() <= 1 = elapsed < 2s のとき true。
            // stale にするには cold_early_1 から 2s 以上待つ必要がある。
            ("MERGE_READY_STALE_TTL", "1"),
            ("MERGE_READY_SCHEDULER_TICK_SECS", "1"),
        ],
    );

    DaemonHandle::wait_for_cache(&env, 5000);

    // warm_to_cold_secs=0 + hot_recent_query_secs=0 → wait_for_cache 直後 ~1s で has_recent_query=false
    // → cold_early_1 リフレッシュが ~T+2s に発生し count=1 → late=60s に切替。
    // cold_early_1 から 2s 以上待ってエントリを stale にする必要がある。
    // sleep(4s): cold_early_1 は ~T+2s, 4s 後の elapsed ≈ 2s → as_secs()=2 > 1 → stale ✓
    std::thread::sleep(std::time::Duration::from_secs(4));

    let calls_before_reset = std::fs::read_to_string(&log_path).unwrap_or_default().len();
    assert!(
        calls_before_reset >= 2,
        "should have initial + at least 1 cold-early refresh (calls: {calls_before_reset})"
    );

    // クエリを送る → エントリが stale → is_cold_or_never_queried(0)=true → reset_cold_count → count=0
    // また NeedsRefresh → mark_refreshing → bg refresh 開始 → 追加 gh 呼び出しあり
    DaemonHandle::wait_for_cache(&env, 5000);

    // レスポンス後に bg refresh が gh を起動してログに書き込むまで少し待つ
    std::thread::sleep(std::time::Duration::from_secs(1));

    let calls_after_reset = std::fs::read_to_string(&log_path).unwrap_or_default().len();
    assert!(
        calls_after_reset > calls_before_reset,
        "reset_cold_count triggers a stale refresh which should produce new gh calls \
         (calls before: {calls_before_reset}, after: {calls_after_reset})"
    );
}
