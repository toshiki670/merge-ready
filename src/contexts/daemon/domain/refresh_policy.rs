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
    #[must_use]
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
    #[must_use]
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
    use super::*;
    use crate::contexts::daemon::domain::cache::{CacheEntry, RefreshMode};
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    fn policy() -> RefreshPolicy {
        RefreshPolicy {
            hot_recent_query_secs: 30,
            hot_with_query_secs: 2,
            hot_without_query_secs: 10,
            warm_refresh_secs: 180,
            warm_to_cold_secs: 1800,
            cold_early_secs: 1800,
            cold_late_secs: 3600,
            cold_early_limit: 10,
        }
    }

    fn make_entry(output: &str, refresh_mode: RefreshMode) -> CacheEntry {
        let mut e = CacheEntry::new(PathBuf::new(), 5);
        e.update(output.to_owned(), refresh_mode);
        e.record_query();
        e
    }

    // ── effective_refresh_interval_secs ──────────────────────────────────────

    #[test]
    fn terminal_interval_is_max() {
        let entry = make_entry("", RefreshMode::Terminal);
        assert_eq!(policy().effective_refresh_interval_secs(&entry), u64::MAX);
    }

    #[test]
    fn hot_with_recent_query_uses_short_interval() {
        let entry = make_entry("⧖ CI", RefreshMode::Hot);
        assert_eq!(
            policy().effective_refresh_interval_secs(&entry),
            policy().hot_with_query_secs
        );
    }

    #[test]
    fn hot_without_recent_query_uses_long_interval() {
        let mut entry = make_entry("⧖ CI", RefreshMode::Hot);
        entry.last_queried_at = Some(
            Instant::now()
                .checked_sub(Duration::from_secs(policy().hot_recent_query_secs + 1))
                .unwrap(),
        );
        assert_eq!(
            policy().effective_refresh_interval_secs(&entry),
            policy().hot_without_query_secs
        );
    }

    #[test]
    fn warm_with_recent_query_promotes_to_hot_interval() {
        let entry = make_entry("✓ Ready", RefreshMode::Warm);
        assert_eq!(
            policy().effective_refresh_interval_secs(&entry),
            policy().hot_with_query_secs
        );
    }

    #[test]
    fn warm_without_recent_query_uses_warm_interval() {
        let mut entry = make_entry("✓ Ready", RefreshMode::Warm);
        entry.last_queried_at = Some(
            Instant::now()
                .checked_sub(Duration::from_secs(policy().hot_recent_query_secs + 1))
                .unwrap(),
        );
        assert_eq!(
            policy().effective_refresh_interval_secs(&entry),
            policy().warm_refresh_secs
        );
    }

    #[test]
    fn warm_cold_early_uses_cold_early_interval() {
        let mut entry = make_entry("✓ Ready", RefreshMode::Warm);
        entry.last_queried_at = Some(
            Instant::now()
                .checked_sub(Duration::from_secs(policy().warm_to_cold_secs + 1))
                .unwrap(),
        );
        entry.cold_refresh_count = 0;
        assert_eq!(
            policy().effective_refresh_interval_secs(&entry),
            policy().cold_early_secs
        );
    }

    #[test]
    fn warm_cold_late_uses_cold_late_interval() {
        let mut entry = make_entry("✓ Ready", RefreshMode::Warm);
        entry.last_queried_at = Some(
            Instant::now()
                .checked_sub(Duration::from_secs(policy().warm_to_cold_secs + 1))
                .unwrap(),
        );
        entry.cold_refresh_count = policy().cold_early_limit;
        assert_eq!(
            policy().effective_refresh_interval_secs(&entry),
            policy().cold_late_secs
        );
    }

    // ── effective_ttl ─────────────────────────────────────────────────────────

    #[test]
    fn terminal_ttl_uses_warm_refresh_secs() {
        let entry = make_entry("", RefreshMode::Terminal);
        assert_eq!(
            policy().effective_ttl(&entry, 5),
            policy().warm_refresh_secs
        );
    }

    #[test]
    fn non_terminal_ttl_uses_base_ttl() {
        let entry = make_entry("✓ Ready", RefreshMode::Warm);
        assert_eq!(policy().effective_ttl(&entry, 5), 5);
    }
}
