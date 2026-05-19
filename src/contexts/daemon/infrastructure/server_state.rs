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
    fn is_backed_off_false_when_unset() {
        let store = CacheStore::new();
        assert!(!store.is_backed_off(Instant::now()));
    }

    #[test]
    fn is_backed_off_true_when_future() {
        let store =
            CacheStore::new().with_backoff_until(Some(Instant::now() + Duration::from_secs(10)));
        assert!(store.is_backed_off(Instant::now()));
    }

    #[test]
    fn is_backed_off_false_when_past() {
        let past = Instant::now()
            .checked_sub(Duration::from_secs(10))
            .expect("past");
        let store = CacheStore::new().with_backoff_until(Some(past));
        assert!(!store.is_backed_off(Instant::now()));
    }
}
