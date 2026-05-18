use std::time::Instant;

use super::server_config::DaemonServerConfig;
use crate::contexts::daemon::domain::cache::CacheStore;

pub(super) struct DaemonState {
    /// ドメインの純粋値オブジェクト。`transition` 純粋関数の入出力で完全に置き換える。
    pub(super) cache_store: CacheStore,
    pub(super) started_at: Instant,
    pub(super) config: DaemonServerConfig,
}

impl DaemonState {
    pub(super) fn new(config: DaemonServerConfig) -> Self {
        Self {
            cache_store: CacheStore::new(),
            started_at: Instant::now(),
            config,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contexts::daemon::domain::cache::CacheStore;
    use std::time::Duration;

    #[test]
    fn should_backoff_false_when_unset() {
        let store = CacheStore::new();
        assert!(!store.should_backoff(Instant::now()));
    }

    #[test]
    fn should_backoff_true_when_future() {
        let mut store = CacheStore::new();
        store.set_backoff(Instant::now() + Duration::from_secs(10));
        assert!(store.should_backoff(Instant::now()));
    }

    #[test]
    fn should_backoff_false_when_past() {
        let mut store = CacheStore::new();
        let past = Instant::now()
            .checked_sub(Duration::from_secs(10))
            .expect("past");
        store.set_backoff(past);
        assert!(!store.should_backoff(Instant::now()));
    }

    #[test]
    fn clear_backoff_if_expired_clears_past() {
        let mut store = CacheStore::new();
        let past = Instant::now()
            .checked_sub(Duration::from_secs(10))
            .expect("past");
        store.set_backoff(past);
        store.clear_backoff_if_expired(Instant::now());
        assert_eq!(store.backoff_until(), None);
    }

    #[test]
    fn clear_backoff_if_expired_keeps_future() {
        let mut store = CacheStore::new();
        let future = Instant::now() + Duration::from_secs(10);
        store.set_backoff(future);
        store.clear_backoff_if_expired(Instant::now());
        assert!(store.backoff_until().is_some());
    }
}
