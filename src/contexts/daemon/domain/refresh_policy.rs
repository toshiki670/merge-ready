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
