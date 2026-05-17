use std::collections::HashMap;
use std::time::Instant;

use super::server_config::DaemonServerConfig;
use crate::contexts::daemon::domain::cache::{CacheEntry, RepoId};
use crate::contexts::daemon::domain::rate_limit_snapshot::RateLimitSnapshot;

pub(super) struct DaemonState {
    pub(super) entries: HashMap<RepoId, CacheEntry>,
    pub(super) started_at: Instant,
    pub(super) config: DaemonServerConfig,
    /// Rate limit 枯渇により全リフレッシュを停止する Instant。
    /// `None` のとき停止しない。`Some(t)` のとき `Instant::now() < t` ならスキップ。
    pub(super) backoff_until: Option<Instant>,
    /// 直近の `gh api rate_limit` スナップショット。`None` は未取得 or 取得失敗中。
    pub(super) latest_rate_limit: Option<RateLimitSnapshot>,
}

impl DaemonState {
    pub(super) fn new(config: DaemonServerConfig) -> Self {
        Self {
            entries: HashMap::new(),
            started_at: Instant::now(),
            config,
            backoff_until: None,
            latest_rate_limit: None,
        }
    }

    /// `now` の時点で backoff 中なら true を返す。
    pub(super) fn should_backoff(&self, now: Instant) -> bool {
        self.backoff_until.is_some_and(|t| now < t)
    }

    /// Rate limit 枯渇を観測した時点で `reset_instant` まで backoff を設定する。
    pub(super) fn set_backoff(&mut self, reset_instant: Instant) {
        self.backoff_until = Some(reset_instant);
    }

    /// backoff が `now` を過ぎていればクリアする。
    pub(super) fn clear_backoff_if_expired(&mut self, now: Instant) {
        if self.backoff_until.is_some_and(|t| now >= t) {
            self.backoff_until = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contexts::daemon::domain::refresh_policy::RefreshPolicy;
    use std::time::Duration;

    fn test_config() -> DaemonServerConfig {
        DaemonServerConfig {
            stale_ttl_secs: 5,
            refresh_lock_timeout_secs: 120,
            entry_max_age_secs: 60,
            scheduler_tick_secs: 2,
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
    fn should_backoff_false_when_unset() {
        let state = DaemonState::new(test_config());
        assert!(!state.should_backoff(Instant::now()));
    }

    #[test]
    fn should_backoff_true_when_future() {
        let mut state = DaemonState::new(test_config());
        state.set_backoff(Instant::now() + Duration::from_secs(10));
        assert!(state.should_backoff(Instant::now()));
    }

    #[test]
    fn should_backoff_false_when_past() {
        let mut state = DaemonState::new(test_config());
        let past = Instant::now()
            .checked_sub(Duration::from_secs(10))
            .expect("past");
        state.set_backoff(past);
        assert!(!state.should_backoff(Instant::now()));
    }

    #[test]
    fn clear_backoff_if_expired_clears_past() {
        let mut state = DaemonState::new(test_config());
        let past = Instant::now()
            .checked_sub(Duration::from_secs(10))
            .expect("past");
        state.set_backoff(past);
        state.clear_backoff_if_expired(Instant::now());
        assert!(state.backoff_until.is_none());
    }

    #[test]
    fn clear_backoff_if_expired_keeps_future() {
        let mut state = DaemonState::new(test_config());
        let future = Instant::now() + Duration::from_secs(10);
        state.set_backoff(future);
        state.clear_backoff_if_expired(Instant::now());
        assert!(state.backoff_until.is_some());
    }
}
