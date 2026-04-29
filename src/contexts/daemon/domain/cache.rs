/// daemon がキャッシュエントリのリフレッシュ頻度を制御するモード。
/// evaluation コンテキストの知識（CI 状態・終端状態）を抽象化した daemon 固有の概念。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshMode {
    /// CI 実行中。素早いリフレッシュが必要。
    Hot,
    /// CI 完了・通常監視中。
    Warm,
    /// PR が merged / closed。リフレッシュ不要。
    Terminal,
}

/// キャッシュの更新ポート
pub trait CachePort {
    /// キャッシュを更新する。失敗は静かに無視する。
    fn update(&self, repo_id: &str, output: &str, refresh_mode: RefreshMode);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    fn make_entry(output: &str, refresh_mode: RefreshMode) -> CacheEntry {
        let mut e = CacheEntry::new(PathBuf::new(), 5);
        e.update(output.to_owned(), refresh_mode);
        e.record_query();
        e
    }

    fn make_stale_entry(output: &str, refresh_mode: RefreshMode, age_secs: u64) -> CacheEntry {
        let mut e = make_entry(output, refresh_mode);
        e.fetched_at = Instant::now()
            .checked_sub(Duration::from_secs(age_secs))
            .unwrap_or_else(Instant::now);
        e
    }

    // ── RepoId ────────────────────────────────────────────────────────────────

    #[test]
    fn repo_id_as_str_returns_inner() {
        let id = RepoId::new("abc123");
        assert_eq!(id.as_str(), "abc123");
    }

    #[test]
    fn repo_id_equality() {
        assert_eq!(RepoId::new("a"), RepoId::new("a"));
        assert_ne!(RepoId::new("a"), RepoId::new("b"));
    }

    #[test]
    fn repo_id_from_and_into_string() {
        let id = RepoId::from("test".to_owned());
        assert_eq!(id.as_str(), "test");
        let s: String = RepoId::new("test").into();
        assert_eq!(s, "test");
    }

    // ── CacheEntry::new ───────────────────────────────────────────────────────

    #[test]
    fn new_entry_starts_refreshing() {
        let e = CacheEntry::new(PathBuf::new(), 5);
        assert!(e.is_refreshing());
    }

    #[test]
    fn new_entry_has_not_fetched() {
        let e = CacheEntry::new(PathBuf::new(), 5);
        assert!(!e.has_fetched());
    }

    #[test]
    fn new_entry_is_stale() {
        let e = CacheEntry::new(PathBuf::new(), 5);
        assert!(!e.is_fresh(5));
    }

    // ── CacheEntry::update ────────────────────────────────────────────────────

    #[test]
    fn update_clears_refreshing_and_sets_has_fetched() {
        let mut e = CacheEntry::new(PathBuf::new(), 5);
        e.update("out".to_owned(), RefreshMode::Warm);
        assert!(!e.is_refreshing());
        assert!(e.has_fetched());
    }

    #[test]
    fn update_sets_fresh() {
        let mut e = CacheEntry::new(PathBuf::new(), 5);
        e.update("out".to_owned(), RefreshMode::Warm);
        assert!(e.is_fresh(5));
    }

    #[test]
    fn update_sets_refresh_mode() {
        let mut e = make_entry("out", RefreshMode::Warm);
        e.update(String::new(), RefreshMode::Terminal);
        assert_eq!(e.refresh_mode(), RefreshMode::Terminal);
    }

    // ── CacheEntry::is_active ─────────────────────────────────────────────────

    #[test]
    fn active_when_non_empty_and_warm() {
        assert!(make_entry("✓ Ready", RefreshMode::Warm).is_active());
    }

    #[test]
    fn active_when_hot() {
        assert!(make_entry("⧖ CI", RefreshMode::Hot).is_active());
    }

    #[test]
    fn inactive_when_empty_output() {
        assert!(!make_entry("", RefreshMode::Warm).is_active());
    }

    #[test]
    fn inactive_when_terminal() {
        assert!(!make_entry("✓ Ready", RefreshMode::Terminal).is_active());
    }

    // ── CacheEntry::reset_to_warm ─────────────────────────────────────────────

    #[test]
    fn reset_to_warm_from_terminal() {
        let mut e = make_entry("out", RefreshMode::Terminal);
        e.reset_to_warm();
        assert_eq!(e.refresh_mode(), RefreshMode::Warm);
    }

    // ── CacheEntry::cold_count ────────────────────────────────────────────────

    #[test]
    fn increment_and_reset_cold_count() {
        let mut e = make_entry("out", RefreshMode::Warm);
        assert_eq!(e.cold_refresh_count(), 0);
        e.increment_cold_count();
        e.increment_cold_count();
        assert_eq!(e.cold_refresh_count(), 2);
        e.reset_cold_count();
        assert_eq!(e.cold_refresh_count(), 0);
    }

    // ── CacheEntry::is_expired ────────────────────────────────────────────────

    #[test]
    fn recently_queried_entry_is_not_expired() {
        let e = make_entry("out", RefreshMode::Warm);
        assert!(!e.is_expired(3600));
    }

    #[test]
    fn old_query_entry_is_expired() {
        let mut e = make_entry("out", RefreshMode::Warm);
        e.last_queried_at = Some(
            Instant::now()
                .checked_sub(Duration::from_secs(3601))
                .unwrap(),
        );
        assert!(e.is_expired(3600));
    }

    // ── CacheEntry::refresh_lock_expired ──────────────────────────────────────

    #[test]
    fn fresh_lock_is_not_expired() {
        let e = CacheEntry::new(PathBuf::new(), 5);
        assert!(!e.refresh_lock_expired(120));
    }

    #[test]
    fn old_lock_is_expired() {
        let mut e = CacheEntry::new(PathBuf::new(), 5);
        e.refresh_started_at = Some(
            Instant::now()
                .checked_sub(Duration::from_secs(121))
                .unwrap(),
        );
        assert!(e.refresh_lock_expired(120));
    }

    // ── CacheEntry::is_cold_or_never_queried ──────────────────────────────────

    #[test]
    fn never_queried_is_cold() {
        let mut e = CacheEntry::new(PathBuf::new(), 5);
        e.last_queried_at = None;
        assert!(e.is_cold_or_never_queried(1800));
    }

    #[test]
    fn recently_queried_is_not_cold() {
        let e = make_entry("out", RefreshMode::Warm);
        assert!(!e.is_cold_or_never_queried(1800));
    }

    // ── CacheEntry::has_recent_query ──────────────────────────────────────────

    #[test]
    fn recent_query_within_threshold() {
        let e = make_entry("out", RefreshMode::Warm);
        assert!(e.has_recent_query(30));
    }

    #[test]
    fn stale_query_outside_threshold() {
        let mut e = make_entry("out", RefreshMode::Warm);
        e.last_queried_at = Some(
            Instant::now()
                .checked_sub(Duration::from_secs(31))
                .unwrap(),
        );
        assert!(!e.has_recent_query(30));
    }

    // ── make_stale_entry (infra tests でも使う共通パターンの検証) ─────────────

    #[test]
    fn stale_entry_is_not_fresh() {
        let e = make_stale_entry("out", RefreshMode::Hot, 9999);
        assert!(!e.is_fresh(5));
    }

    // ── CacheEntry::clear_refresh_lock ────────────────────────────────────────

    #[test]
    fn clear_refresh_lock_resets_refreshing() {
        let mut e = CacheEntry::new(PathBuf::new(), 5);
        assert!(e.is_refreshing());
        e.clear_refresh_lock();
        assert!(!e.is_refreshing());
        assert!(!e.refresh_lock_expired(120));
    }
}
