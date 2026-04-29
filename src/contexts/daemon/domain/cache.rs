use std::path::PathBuf;
use std::time::{Duration, Instant};

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

/// リポジトリとブランチの組み合わせを識別する値オブジェクト。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RepoId(String);

impl RepoId {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for RepoId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<RepoId> for String {
    fn from(r: RepoId) -> Self {
        r.0
    }
}

impl std::fmt::Display for RepoId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// キャッシュエントリのドメインエンティティ。
///
/// 1 リポジトリ＋ブランチに対応するキャッシュ状態を保持し、
/// 状態遷移（update・mark_refreshing・record_query 等）と
/// 状態クエリ（is_active・is_fresh・is_expired 等）を提供する。
pub struct CacheEntry {
    output: String,
    has_fetched: bool,
    pub(crate) fetched_at: Instant,
    refreshing: bool,
    pub(crate) refresh_started_at: Option<Instant>,
    pub(crate) cwd: PathBuf,
    refresh_mode: RefreshMode,
    pub(crate) last_queried_at: Option<Instant>,
    pub(crate) cold_refresh_count: u32,
}

impl CacheEntry {
    /// 初回ミス時に生成する新規エントリ。即リフレッシュ済み状態でマークする。
    ///
    /// `stale_ttl` は `fetched_at` を TTL 超過済みの過去時刻にセットするために使う。
    pub fn new(cwd: PathBuf, stale_ttl: u64) -> Self {
        let past = Instant::now()
            .checked_sub(Duration::from_secs(stale_ttl.saturating_add(1)))
            .unwrap_or_else(Instant::now);
        Self {
            output: String::new(),
            has_fetched: false,
            fetched_at: past,
            refreshing: true,
            refresh_started_at: Some(Instant::now()),
            cwd,
            refresh_mode: RefreshMode::Warm,
            last_queried_at: Some(Instant::now()),
            cold_refresh_count: 0,
        }
    }

    // ── 状態遷移 ──────────────────────────────────────────────────────────────

    /// バックグラウンドワーカーの取得結果でエントリを更新する。
    pub fn update(&mut self, output: String, refresh_mode: RefreshMode) {
        self.output = output;
        self.has_fetched = true;
        self.fetched_at = Instant::now();
        self.refreshing = false;
        self.refresh_started_at = None;
        self.refresh_mode = refresh_mode;
    }

    /// リフレッシュ開始をマークする。
    pub fn mark_refreshing(&mut self) {
        self.refreshing = true;
        self.refresh_started_at = Some(Instant::now());
    }

    /// Query 受付時刻を記録する（Cold 判定・Hot 昇格の基準）。
    pub fn record_query(&mut self) {
        self.last_queried_at = Some(Instant::now());
    }

    /// Cold カウンタをリセットする（Query で Warm に戻ったとき）。
    pub fn reset_cold_count(&mut self) {
        self.cold_refresh_count = 0;
    }

    /// Cold カウンタをインクリメントする（Cold モードでリフレッシュするとき）。
    pub fn increment_cold_count(&mut self) {
        self.cold_refresh_count = self.cold_refresh_count.saturating_add(1);
    }

    /// Terminal → Warm にリセットする（PR 再オープン検知時）。
    pub fn reset_to_warm(&mut self) {
        self.refresh_mode = RefreshMode::Warm;
    }

    /// リフレッシュロックを解除する（タイムアウト時）。
    pub fn clear_refresh_lock(&mut self) {
        self.refreshing = false;
        self.refresh_started_at = None;
    }

    // ── アクセサ ──────────────────────────────────────────────────────────────

    pub fn output(&self) -> &str {
        &self.output
    }

    pub fn has_fetched(&self) -> bool {
        self.has_fetched
    }

    pub fn cwd(&self) -> &std::path::Path {
        &self.cwd
    }

    pub fn refresh_mode(&self) -> RefreshMode {
        self.refresh_mode
    }

    pub fn is_refreshing(&self) -> bool {
        self.refreshing
    }

    pub fn cold_refresh_count(&self) -> u32 {
        self.cold_refresh_count
    }

    // ── 状態クエリ ────────────────────────────────────────────────────────────

    /// 出力が存在し Terminal でないとき active とみなす（バックグラウンドリフレッシュ対象）。
    pub fn is_active(&self) -> bool {
        !self.output.is_empty() && self.refresh_mode != RefreshMode::Terminal
    }

    /// `fetched_at` から `ttl` 秒以内なら fresh とみなす。
    pub fn is_fresh(&self, ttl: u64) -> bool {
        self.fetched_at.elapsed().as_secs() <= ttl
    }

    /// `last_queried_at` から `max_age_secs` 以上経過したエントリを削除対象とみなす。
    pub fn is_expired(&self, max_age_secs: u64) -> bool {
        self.last_queried_at
            .is_some_and(|t| t.elapsed().as_secs() >= max_age_secs)
    }

    /// リフレッシュ開始から `timeout_secs` 以上経過したらロック切れとみなす。
    pub fn refresh_lock_expired(&self, timeout_secs: u64) -> bool {
        self.refresh_started_at
            .is_some_and(|started| started.elapsed().as_secs() >= timeout_secs)
    }

    /// `last_queried_at` が未設定、または `warm_to_cold_secs` 以上経過していれば Cold とみなす。
    ///
    /// `record_query()` を呼ぶ前に評価すること（呼び後は必ず false になる）。
    pub fn is_cold_or_never_queried(&self, warm_to_cold_secs: u64) -> bool {
        self.last_queried_at
            .is_none_or(|t| t.elapsed().as_secs() >= warm_to_cold_secs)
    }

    /// `last_queried_at` が `recent_secs` 以内なら recent とみなす（Hot 昇格判定）。
    pub fn has_recent_query(&self, recent_secs: u64) -> bool {
        self.last_queried_at
            .is_some_and(|t| t.elapsed().as_secs() <= recent_secs)
    }

    /// `last_queried_at` が `warm_to_cold_secs` 以上経過しているか（is_cold のエイリアス）。
    pub fn is_cold(&self, warm_to_cold_secs: u64) -> bool {
        self.last_queried_at
            .is_some_and(|t| t.elapsed().as_secs() >= warm_to_cold_secs)
    }
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
