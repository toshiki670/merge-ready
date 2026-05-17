//! Issue #274: rate_limit-aware なリフレッシュ調整の E2E。
//!
//! - 既定（`MERGE_READY_RATE_LIMIT_AWARE` 未指定）で daemon が `gh api rate_limit`
//!   を呼んでいることを確認する
//! - `MERGE_READY_RATE_LIMIT_AWARE=0` で `gh api rate_limit` が呼ばれないことを確認する
//! - 枯渇スナップショット（`remaining=0`）を返すと、reset 時刻まで `pr list` が
//!   呼ばれなくなり、reset 経過後に再開する

use super::super::helpers::DaemonHandle;
use super::rate_limit_aware_fixtures;

/// 既定では daemon は `gh api rate_limit` を定期的に呼ぶ。
#[test]
fn test_rate_limit_aware_default_calls_rate_limit_endpoint() {
    let fx = rate_limit_aware_fixtures::with_rate_limit_response(10_000, 3600);
    let _daemon = DaemonHandle::start_with_env(
        &fx.env,
        &[
            ("MERGE_READY_STALE_TTL", "60"),
            ("MERGE_READY_SCHEDULER_TICK_SECS", "1"),
        ],
    );

    DaemonHandle::wait_for_cache(&fx.env, 15000);
    std::thread::sleep(std::time::Duration::from_secs(1));

    let calls = std::fs::read_to_string(&fx.rate_limit_log)
        .unwrap_or_default()
        .len();
    assert!(
        calls >= 1,
        "rate_limit_aware default ON: gh api rate_limit must be called at least once (calls: {calls})"
    );
}

/// `MERGE_READY_RATE_LIMIT_AWARE=0` のとき `gh api rate_limit` は呼ばれない。
#[test]
fn test_rate_limit_aware_disabled_does_not_call_rate_limit_endpoint() {
    let fx = rate_limit_aware_fixtures::with_rate_limit_response(10_000, 3600);
    let _daemon = DaemonHandle::start_with_env(
        &fx.env,
        &[
            ("MERGE_READY_RATE_LIMIT_AWARE", "0"),
            ("MERGE_READY_STALE_TTL", "60"),
            ("MERGE_READY_SCHEDULER_TICK_SECS", "1"),
        ],
    );

    DaemonHandle::wait_for_cache(&fx.env, 15000);
    std::thread::sleep(std::time::Duration::from_secs(1));

    let calls = std::fs::read_to_string(&fx.rate_limit_log)
        .unwrap_or_default()
        .len();
    assert_eq!(
        calls, 0,
        "rate_limit_aware OFF: gh api rate_limit must not be called (calls: {calls})"
    );
}

/// クォータ枯渇スナップショットを観測すると daemon が backoff に入り、
/// reset まで `pr list` が呼ばれなくなる。reset 経過後に再開する。
///
/// fake `gh` は 1 回目の `api rate_limit` で枯渇、2 回目以降で残量フルを返す。
/// `rate_limit` fetch 間隔を 1 秒に短縮することで、reset 経過後の
/// 「次の snapshot 更新 → backoff 状態解除」までを 3 秒以内に確認できる。
#[test]
fn test_rate_limit_exhausted_pauses_refresh_until_reset() {
    // 1 回目: remaining=0, reset=now+2s。2 回目以降: 残量フル。
    let fx = rate_limit_aware_fixtures::with_rate_limit_exhaust_then_recover(2);
    let _daemon = DaemonHandle::start_with_env(
        &fx.env,
        &[
            ("MERGE_READY_HOT_RECENT_QUERY_SECS", "60"),
            ("MERGE_READY_HOT_WITH_QUERY_SECS", "1"),
            ("MERGE_READY_WARM_REFRESH_SECS", "1"),
            ("MERGE_READY_STALE_TTL", "60"),
            ("MERGE_READY_SCHEDULER_TICK_SECS", "1"),
            ("MERGE_READY_RATE_LIMIT_FETCH_INTERVAL_SECS", "1"),
        ],
    );

    DaemonHandle::wait_for_cache(&fx.env, 15000);

    // backoff 中 (~2s 間) は pr list 呼び出しが発生しないはず
    let pr_calls_at_t0 = std::fs::read_to_string(&fx.pr_list_log)
        .unwrap_or_default()
        .len();
    std::thread::sleep(std::time::Duration::from_millis(1500));
    let pr_calls_during_backoff = std::fs::read_to_string(&fx.pr_list_log)
        .unwrap_or_default()
        .len();
    assert_eq!(
        pr_calls_at_t0, pr_calls_during_backoff,
        "during backoff (~1.5s after exhaustion observed), pr list should not be called \
         (t0: {pr_calls_at_t0}, t0+1.5s: {pr_calls_during_backoff})"
    );

    // reset 経過 + 次の rate_limit fetch (1s 間隔) + 次の scheduler tick で再開する
    std::thread::sleep(std::time::Duration::from_secs(4));
    let pr_calls_after_reset = std::fs::read_to_string(&fx.pr_list_log)
        .unwrap_or_default()
        .len();
    assert!(
        pr_calls_after_reset > pr_calls_during_backoff,
        "after reset (~5.5s total), refresh must resume \
         (during backoff: {pr_calls_during_backoff}, after reset: {pr_calls_after_reset})"
    );
}
