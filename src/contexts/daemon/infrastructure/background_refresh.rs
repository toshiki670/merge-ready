use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime};

use super::server_state::DaemonState;
use crate::contexts::daemon::domain::cache::{CacheEntry, RepoId};
use crate::shared::refresh_mode::RefreshMode;

/// 全 active エントリの「1 リフレッシュ」当たり API コスト総和を求める。
/// `pr_count + 2`（pr list 1 + repo view 1 + pr checks N）を集計し、
/// Terminal エントリは除外する（`is_active()` で判定）。
fn total_refresh_cost<'a, I>(entries: I) -> u64
where
    I: IntoIterator<Item = &'a CacheEntry>,
{
    let mut total: u64 = 0;
    for entry in entries {
        if !entry.is_active() {
            continue;
        }
        let pr_count = u64::try_from(entry.pr_outputs().len()).unwrap_or(u64::MAX);
        total = total.saturating_add(pr_count.saturating_add(2));
    }
    total
}

pub(super) fn collect_targets(state: &Arc<Mutex<DaemonState>>) -> Vec<(RepoId, PathBuf)> {
    let mut s = state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let config = s.config;
    let policy = config.policy;

    // 期限切れ backoff のクリーンアップ
    s.clear_backoff_if_expired(Instant::now());

    // backoff 中はリフレッシュを一切行わない（rate_limit 枯渇時の保護）
    if s.should_backoff(Instant::now()) {
        return Vec::new();
    }

    s.entries
        .retain(|_, entry| !entry.is_expired(config.entry_max_age_secs));

    let total_cost = total_refresh_cost(s.entries.values());
    let snapshot = if config.rate_limit_aware {
        s.latest_rate_limit
    } else {
        None
    };
    let now_wall = SystemTime::now();

    let mut targets = Vec::new();
    for (repo_id, entry) in &mut s.entries {
        if !entry.is_active() {
            continue;
        }
        if entry.is_refreshing() && entry.refresh_lock_expired(config.refresh_lock_timeout_secs) {
            entry.clear_refresh_lock();
        }
        if entry.is_refreshing() {
            continue;
        }
        let interval = policy.effective_refresh_interval_secs_scaled(
            entry,
            snapshot.as_ref(),
            total_cost,
            now_wall,
        );
        if entry.fetched_at_elapsed().as_secs() < interval {
            continue;
        }
        if entry.refresh_mode() == RefreshMode::Warm && entry.is_cold(policy.warm_to_cold_secs) {
            entry.increment_cold_count();
        }
        entry.mark_refreshing();
        targets.push((repo_id.clone(), entry.cwd().to_path_buf()));
    }
    targets
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::protocol::PrOutput;
    use std::time::Duration;

    fn entry_with_prs(pr_count: usize, active: bool) -> CacheEntry {
        let mut e = CacheEntry::new(PathBuf::from("/tmp"), "main".to_owned(), 5);
        let prs: Vec<PrOutput> = (0..pr_count)
            .map(|i| PrOutput {
                pr_id: i as u64,
                output: String::new(),
            })
            .collect();
        let mode = if active {
            RefreshMode::Warm
        } else {
            RefreshMode::Terminal
        };
        e.update("output".to_owned(), prs, mode);
        e
    }

    #[test]
    fn total_refresh_cost_excludes_terminal() {
        let entries = vec![
            entry_with_prs(2, true),  // active, cost = 2 + 2 = 4
            entry_with_prs(0, true),  // active, cost = 0 + 2 = 2
            entry_with_prs(3, false), // terminal, excluded
        ];
        assert_eq!(total_refresh_cost(&entries), 6);
    }

    #[test]
    fn total_refresh_cost_excludes_loading() {
        // is_active() は output 空も除外する
        let mut e = CacheEntry::new(PathBuf::from("/tmp"), "main".to_owned(), 5);
        // update せず Loading 状態のまま → output 空
        assert!(!e.is_active());
        // update して active に
        e.update("o".to_owned(), Vec::new(), RefreshMode::Warm);
        assert!(e.is_active());
        let entries = vec![e];
        assert_eq!(total_refresh_cost(&entries), 2);
    }

    #[test]
    fn total_refresh_cost_zero_when_all_terminal() {
        let entries = vec![entry_with_prs(1, false), entry_with_prs(2, false)];
        assert_eq!(total_refresh_cost(&entries), 0);
    }

    // ── collect_targets の backoff スキップ ───────────────────────────────────

    use super::super::server_config::DaemonServerConfig;
    use crate::contexts::daemon::domain::refresh_policy::RefreshPolicy;

    fn test_config() -> DaemonServerConfig {
        DaemonServerConfig {
            stale_ttl_secs: 5,
            refresh_lock_timeout_secs: 120,
            entry_max_age_secs: 60,
            scheduler_tick_secs: 2,
            socket_check_interval_secs: 5,
            policy: RefreshPolicy {
                hot_recent_query_secs: 30,
                hot_with_query_secs: 2,
                hot_without_query_secs: 10,
                warm_refresh_secs: 180,
                warm_to_cold_secs: 1800,
                cold_early_secs: 1800,
                cold_late_secs: 3600,
                cold_early_limit: 10,
            },
            rate_limit_aware: true,
            rate_limit_fetch_interval_secs: 60,
        }
    }

    #[test]
    fn collect_targets_returns_empty_when_in_backoff() {
        let mut state = DaemonState::new(test_config());
        state
            .entries
            .insert(RepoId::new("test".to_owned()), entry_with_prs(1, true));
        // backoff を未来に設定
        state.set_backoff(Instant::now() + Duration::from_mins(1));
        let state = Arc::new(Mutex::new(state));
        let targets = collect_targets(&state);
        assert!(targets.is_empty());
    }

    #[test]
    fn collect_targets_clears_expired_backoff() {
        let mut state = DaemonState::new(test_config());
        // 既に期限切れ
        let past = Instant::now()
            .checked_sub(Duration::from_secs(10))
            .expect("past");
        state.set_backoff(past);
        let state = Arc::new(Mutex::new(state));
        let _ = collect_targets(&state);
        let s = state.lock().unwrap();
        assert!(
            s.backoff_until.is_none(),
            "expired backoff should be cleared"
        );
    }
}
