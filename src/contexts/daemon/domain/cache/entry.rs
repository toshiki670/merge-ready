use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

use crate::shared::protocol::PrOutput;
use crate::shared::refresh_mode::RefreshMode;

/// `CacheEntry` のフェッチ状態を表す状態機械。
///
/// 有効な遷移:
/// `Loading` → (`update`) → `Ready` → (`mark_refreshing`) → `Refreshing` → (`update`) → `Ready`
/// `Loading` → (`clear_refresh_lock`) → `PendingRetry` → (`mark_refreshing`) → `Loading`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FetchState {
    /// 初回リフレッシュ進行中。データ未取得。
    Loading,
    /// 初回リフレッシュがタイムアウト。再スケジュール待ち。データ未取得。
    PendingRetry,
    /// データ取得済み。リフレッシュ不要。
    Ready,
    /// データ取得済み。再リフレッシュ進行中。
    Refreshing,
}

/// キャッシュエントリのドメインエンティティ。
pub struct CacheEntry {
    output: String,
    pr_outputs: Vec<PrOutput>,
    fetch_state: FetchState,
    pub(crate) fetched_at: Instant,
    fetched_at_wall: SystemTime,
    pub(crate) refresh_started_at: Option<Instant>,
    pub(crate) cwd: PathBuf,
    branch: String,
    refresh_mode: RefreshMode,
    pub(crate) last_queried_at: Option<Instant>,
    pub(crate) cold_refresh_count: u32,
}

impl CacheEntry {
    /// 初回ミス時に生成する新規エントリ。即リフレッシュ済み状態でマークする。
    ///
    /// `stale_ttl` は `fetched_at` を TTL 超過済みの過去時刻にセットするために使う。
    pub fn new(cwd: PathBuf, branch: String, stale_ttl: u64) -> Self {
        let past = Instant::now()
            .checked_sub(Duration::from_secs(stale_ttl.saturating_add(1)))
            .unwrap_or_else(Instant::now);
        Self {
            output: String::new(),
            pr_outputs: Vec::new(),
            fetch_state: FetchState::Loading,
            fetched_at: past,
            fetched_at_wall: SystemTime::now(),
            refresh_started_at: Some(Instant::now()),
            cwd,
            branch,
            refresh_mode: RefreshMode::Warm,
            last_queried_at: Some(Instant::now()),
            cold_refresh_count: 0,
        }
    }

    /// バックグラウンドワーカーの取得結果でエントリを更新する。
    pub fn update(&mut self, output: String, pr_outputs: Vec<PrOutput>, refresh_mode: RefreshMode) {
        self.output = output;
        self.pr_outputs = pr_outputs;
        self.fetch_state = FetchState::Ready;
        self.fetched_at = Instant::now();
        self.fetched_at_wall = SystemTime::now();
        self.refresh_started_at = None;
        self.refresh_mode = refresh_mode;
    }

    pub fn pr_outputs(&self) -> &[PrOutput] {
        &self.pr_outputs
    }

    /// リフレッシュ開始をマークする。
    pub fn mark_refreshing(&mut self) {
        self.fetch_state = match self.fetch_state {
            FetchState::Loading | FetchState::PendingRetry => FetchState::Loading,
            FetchState::Ready | FetchState::Refreshing => FetchState::Refreshing,
        };
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
        self.fetch_state = match self.fetch_state {
            FetchState::Loading | FetchState::PendingRetry => FetchState::PendingRetry,
            FetchState::Ready | FetchState::Refreshing => FetchState::Ready,
        };
        self.refresh_started_at = None;
    }

    pub fn output(&self) -> &str {
        &self.output
    }

    pub fn has_fetched(&self) -> bool {
        matches!(self.fetch_state, FetchState::Ready | FetchState::Refreshing)
    }

    pub fn cwd(&self) -> &std::path::Path {
        &self.cwd
    }

    pub fn branch(&self) -> &str {
        &self.branch
    }

    pub fn fetched_at_wall(&self) -> SystemTime {
        self.fetched_at_wall
    }

    pub fn refresh_mode(&self) -> RefreshMode {
        self.refresh_mode
    }

    pub fn is_refreshing(&self) -> bool {
        matches!(
            self.fetch_state,
            FetchState::Loading | FetchState::Refreshing
        )
    }

    pub fn cold_refresh_count(&self) -> u32 {
        self.cold_refresh_count
    }

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

    /// `last_queried_at` が `warm_to_cold_secs` 以上経過しているか（queried 前提版、never queried は false）。
    pub fn is_cold(&self, warm_to_cold_secs: u64) -> bool {
        self.last_queried_at
            .is_some_and(|t| t.elapsed().as_secs() >= warm_to_cold_secs)
    }
}
