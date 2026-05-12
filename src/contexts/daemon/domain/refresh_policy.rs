use crate::contexts::daemon::domain::cache::{CacheEntry, RefreshMode};

/// Hot/Warm/Cold 各モードのリフレッシュ間隔と TTL ルールを保持するドメインサービス。
#[derive(Debug, Clone, Copy)]
pub struct RefreshPolicy {
    /// "最近 Query あり" とみなす経過秒数（Hot/Warm 共通）
    pub hot_recent_query_secs: u64,
    /// Hot または Warm + 最近 Query あり の場合のリフレッシュ間隔
    pub hot_with_query_secs: u64,
    /// Hot（Query なし）の場合のリフレッシュ間隔
    pub hot_without_query_secs: u64,
    /// Warm モードの標準リフレッシュ間隔
    pub warm_refresh_secs: u64,
    /// Warm から Cold へ移行するまでの Query 無し経過秒数
    pub warm_to_cold_secs: u64,
    /// Cold 初期（累計リフレッシュ `cold_early_limit` 回まで）の間隔
    pub cold_early_secs: u64,
    /// Cold 後期（`cold_early_limit` 回超）の間隔
    pub cold_late_secs: u64,
    /// Cold 初期から後期へ切り替わる累計リフレッシュ回数
    pub cold_early_limit: u32,
}

impl RefreshPolicy {
    /// エントリの現在の状態からリフレッシュ間隔（秒）を返す。
    pub fn effective_refresh_interval_secs(&self, entry: &CacheEntry) -> u64 {
        match entry.refresh_mode() {
            RefreshMode::Terminal => u64::MAX,
            RefreshMode::Hot => {
                if entry.has_recent_query(self.hot_recent_query_secs) {
                    self.hot_with_query_secs
                } else {
                    self.hot_without_query_secs
                }
            }
            RefreshMode::Warm => {
                if entry.has_recent_query(self.hot_recent_query_secs) {
                    self.hot_with_query_secs
                } else if entry.is_cold(self.warm_to_cold_secs) {
                    self.cold_interval_secs(entry.cold_refresh_count())
                } else {
                    self.warm_refresh_secs
                }
            }
        }
    }

    /// Terminal エントリは `warm_refresh_secs` を TTL として返す（PR 再オープン検知のため）。
    pub fn effective_ttl(&self, entry: &CacheEntry, base_ttl: u64) -> u64 {
        if entry.refresh_mode() == RefreshMode::Terminal {
            self.warm_refresh_secs
        } else {
            base_ttl
        }
    }

    fn cold_interval_secs(&self, count: u32) -> u64 {
        if count < self.cold_early_limit {
            self.cold_early_secs
        } else {
            self.cold_late_secs
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::contexts::daemon::domain::cache::{CacheEntry, RefreshMode};

    use super::RefreshPolicy;

    fn policy() -> RefreshPolicy {
        RefreshPolicy {
            hot_recent_query_secs: 60,
            hot_with_query_secs: 10,
            hot_without_query_secs: 30,
            warm_refresh_secs: 120,
            warm_to_cold_secs: 300,
            cold_early_secs: 600,
            cold_late_secs: 3600,
            cold_early_limit: 5,
        }
    }

    fn entry_with_mode(mode: RefreshMode) -> CacheEntry {
        let mut e = CacheEntry::new(PathBuf::from("/tmp/test"), "branch".into(), 0);
        e.update("output".into(), vec![], mode);
        e
    }

    #[test]
    fn terminal_returns_u64_max() {
        let p = policy();
        let e = entry_with_mode(RefreshMode::Terminal);
        assert_eq!(p.effective_refresh_interval_secs(&e), u64::MAX);
    }

    #[test]
    fn hot_with_recent_query_returns_hot_with_query_secs() {
        let mut p = policy();
        p.hot_recent_query_secs = u64::MAX;
        let e = entry_with_mode(RefreshMode::Hot);
        assert_eq!(p.effective_refresh_interval_secs(&e), p.hot_with_query_secs);
    }

    #[test]
    fn warm_with_recent_query_returns_hot_with_query_secs() {
        let mut p = policy();
        p.hot_recent_query_secs = u64::MAX;
        let e = entry_with_mode(RefreshMode::Warm);
        assert_eq!(p.effective_refresh_interval_secs(&e), p.hot_with_query_secs);
    }

    #[test]
    fn effective_ttl_terminal_returns_warm_refresh_secs() {
        let p = policy();
        let e = entry_with_mode(RefreshMode::Terminal);
        assert_eq!(p.effective_ttl(&e, 999), p.warm_refresh_secs);
    }

    #[test]
    fn effective_ttl_non_terminal_returns_base_ttl() {
        let p = policy();
        let e = entry_with_mode(RefreshMode::Warm);
        assert_eq!(p.effective_ttl(&e, 999), 999);
    }

    // These tests need last_queried_at to be > 0 seconds old.
    // CacheEntry::new sets last_queried_at = Some(Instant::now()), so we sleep 1 second.
    // nextest runs tests in parallel processes, so all 1-second tests complete in ~1s total.

    #[test]
    fn hot_without_recent_query_returns_hot_without_query_secs() {
        let mut p = policy();
        p.hot_recent_query_secs = 0;
        let e = entry_with_mode(RefreshMode::Hot);
        std::thread::sleep(std::time::Duration::from_secs(1));
        assert_eq!(
            p.effective_refresh_interval_secs(&e),
            p.hot_without_query_secs
        );
    }

    #[test]
    fn warm_not_cold_returns_warm_refresh_secs() {
        let mut p = policy();
        p.hot_recent_query_secs = 0;
        p.warm_to_cold_secs = u64::MAX;
        let e = entry_with_mode(RefreshMode::Warm);
        std::thread::sleep(std::time::Duration::from_secs(1));
        assert_eq!(p.effective_refresh_interval_secs(&e), p.warm_refresh_secs);
    }

    #[test]
    fn warm_cold_early_returns_cold_early_secs() {
        let mut p = policy();
        p.hot_recent_query_secs = 0;
        p.warm_to_cold_secs = 0;
        p.cold_early_limit = 5;
        // cold_refresh_count starts at 0 < 5 → early branch
        let e = entry_with_mode(RefreshMode::Warm);
        std::thread::sleep(std::time::Duration::from_secs(1));
        assert_eq!(p.effective_refresh_interval_secs(&e), p.cold_early_secs);
    }

    #[test]
    fn warm_cold_late_returns_cold_late_secs() {
        let mut p = policy();
        p.hot_recent_query_secs = 0;
        p.warm_to_cold_secs = 0;
        p.cold_early_limit = 0;
        // cold_refresh_count starts at 0 >= 0 → late branch
        let e = entry_with_mode(RefreshMode::Warm);
        std::thread::sleep(std::time::Duration::from_secs(1));
        assert_eq!(p.effective_refresh_interval_secs(&e), p.cold_late_secs);
    }
}
