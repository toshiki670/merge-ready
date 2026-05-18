use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

use crate::shared::protocol::PrOutput;
use crate::shared::refresh_mode::RefreshMode;

/// `CacheEntryState` のフェッチ状態を表す状態機械。
///
/// 有効な遷移:
/// `Loading` → (`with_refresh_completed`) → `Ready` → (`with_mark_refreshing`) → `Refreshing` → (`with_refresh_completed`) → `Ready`
/// `Loading` → (`with_clear_refresh_lock`) → `PendingRetry` → (`with_mark_refreshing`) → `Loading`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetchState {
    /// 初回リフレッシュ進行中。データ未取得。
    Loading,
    /// 初回リフレッシュがタイムアウト。再スケジュール待ち。データ未取得。
    PendingRetry,
    /// データ取得済み。リフレッシュ不要。
    Ready,
    /// データ取得済み。再リフレッシュ進行中。
    Refreshing,
}

/// キャッシュエントリのドメインエンティティ。全フィールド private で、
/// 状態遷移は `transition` モジュールの純粋関数経由でのみ起きる。
#[derive(Debug, Clone)]
pub struct CacheEntryState {
    output: String,
    pr_outputs: Vec<PrOutput>,
    fetch_state: FetchState,
    fetched_at: Instant,
    fetched_at_wall: SystemTime,
    refresh_started_at: Option<Instant>,
    cwd: PathBuf,
    branch: String,
    refresh_mode: RefreshMode,
    last_queried_at: Option<Instant>,
    cold_refresh_count: u32,
}

impl CacheEntryState {
    // ── 純粋 getter ─────────────────────────────────────────────

    pub fn output(&self) -> &str {
        &self.output
    }

    pub fn pr_outputs(&self) -> &[PrOutput] {
        &self.pr_outputs
    }

    pub fn cwd(&self) -> &std::path::Path {
        &self.cwd
    }

    pub fn branch(&self) -> &str {
        &self.branch
    }

    pub fn refresh_mode(&self) -> RefreshMode {
        self.refresh_mode
    }

    #[cfg(test)]
    pub fn fetch_state(&self) -> FetchState {
        self.fetch_state
    }

    pub fn fetched_at(&self) -> Instant {
        self.fetched_at
    }

    pub fn fetched_at_wall(&self) -> SystemTime {
        self.fetched_at_wall
    }

    pub fn last_queried_at(&self) -> Option<Instant> {
        self.last_queried_at
    }

    pub fn refresh_started_at(&self) -> Option<Instant> {
        self.refresh_started_at
    }

    pub fn cold_refresh_count(&self) -> u32 {
        self.cold_refresh_count
    }

    // ── 状態に基づく純粋判定（時刻不要） ───────────────────────

    pub fn has_fetched(&self) -> bool {
        matches!(self.fetch_state, FetchState::Ready | FetchState::Refreshing)
    }

    pub fn is_refreshing(&self) -> bool {
        matches!(
            self.fetch_state,
            FetchState::Loading | FetchState::Refreshing
        )
    }

    /// 出力が存在し Terminal でないとき active とみなす（バックグラウンドリフレッシュ対象）。
    pub fn is_active(&self) -> bool {
        !self.output.is_empty() && self.refresh_mode != RefreshMode::Terminal
    }

    // ───────────────────────────────────────────────────────────────
    // 純粋ビルダ（`transition` モジュールから呼び出される）
    // ───────────────────────────────────────────────────────────────

    /// 初回ミス時の純粋コンストラクタ。`fetched_at` を `stale_ttl + 1` 秒前にセットして
    /// 即リフレッシュ対象とする。
    pub(in crate::contexts::daemon::domain) fn new_loading(
        cwd: PathBuf,
        branch: String,
        stale_ttl: u64,
        now: Instant,
        now_wall: SystemTime,
    ) -> Self {
        let past = now
            .checked_sub(Duration::from_secs(stale_ttl.saturating_add(1)))
            .unwrap_or(now);
        Self {
            output: String::new(),
            pr_outputs: Vec::new(),
            fetch_state: FetchState::Loading,
            fetched_at: past,
            fetched_at_wall: now_wall,
            refresh_started_at: Some(now),
            cwd,
            branch,
            refresh_mode: RefreshMode::Warm,
            last_queried_at: Some(now),
            cold_refresh_count: 0,
        }
    }

    /// Query 受付時刻を `now` に更新する。
    pub(super) fn with_record_query(mut self, now: Instant) -> Self {
        self.last_queried_at = Some(now);
        self
    }

    /// リフレッシュ開始をマーク。
    pub(super) fn with_mark_refreshing(mut self, now: Instant) -> Self {
        self.fetch_state = match self.fetch_state {
            FetchState::Loading | FetchState::PendingRetry => FetchState::Loading,
            FetchState::Ready | FetchState::Refreshing => FetchState::Refreshing,
        };
        self.refresh_started_at = Some(now);
        self
    }

    /// リフレッシュロックを解除（タイムアウト時）。
    pub(super) fn with_clear_refresh_lock(mut self) -> Self {
        self.fetch_state = match self.fetch_state {
            FetchState::Loading | FetchState::PendingRetry => FetchState::PendingRetry,
            FetchState::Ready | FetchState::Refreshing => FetchState::Ready,
        };
        self.refresh_started_at = None;
        self
    }

    /// Cold カウンタをリセット（Query で Warm に戻ったとき）。
    pub(super) fn with_reset_cold_count(mut self) -> Self {
        self.cold_refresh_count = 0;
        self
    }

    /// Cold カウンタをインクリメント（Cold モードでリフレッシュするとき）。
    pub(super) fn with_increment_cold_count(mut self) -> Self {
        self.cold_refresh_count = self.cold_refresh_count.saturating_add(1);
        self
    }

    /// Terminal → Warm にリセット（PR 再オープン検知時）。
    pub(super) fn with_reset_to_warm(mut self) -> Self {
        self.refresh_mode = RefreshMode::Warm;
        self
    }

    /// バックグラウンドリフレッシュ完了時の差分を反映。
    pub(in crate::contexts::daemon::domain) fn with_refresh_completed(
        mut self,
        output: String,
        pr_outputs: Vec<PrOutput>,
        refresh_mode: RefreshMode,
        now: Instant,
        now_wall: SystemTime,
    ) -> Self {
        self.output = output;
        self.pr_outputs = pr_outputs;
        self.fetch_state = FetchState::Ready;
        self.fetched_at = now;
        self.fetched_at_wall = now_wall;
        self.refresh_started_at = None;
        self.refresh_mode = refresh_mode;
        self
    }

    #[cfg(test)]
    pub(in crate::contexts::daemon) fn builder_for_test(
        now: Instant,
        now_wall: SystemTime,
    ) -> CacheEntryStateBuilder {
        CacheEntryStateBuilder::new(now, now_wall)
    }
}

#[cfg(test)]
pub(in crate::contexts::daemon) struct CacheEntryStateBuilder {
    output: String,
    pr_outputs: Vec<PrOutput>,
    fetch_state: FetchState,
    fetched_at: Instant,
    fetched_at_wall: SystemTime,
    refresh_started_at: Option<Instant>,
    cwd: PathBuf,
    branch: String,
    refresh_mode: RefreshMode,
    last_queried_at: Option<Instant>,
    cold_refresh_count: u32,
}

#[cfg(test)]
impl CacheEntryStateBuilder {
    fn new(now: Instant, now_wall: SystemTime) -> Self {
        Self {
            output: String::new(),
            pr_outputs: Vec::new(),
            fetch_state: FetchState::Ready,
            fetched_at: now,
            fetched_at_wall: now_wall,
            refresh_started_at: None,
            cwd: PathBuf::from("/tmp"),
            branch: "main".to_owned(),
            refresh_mode: RefreshMode::Warm,
            last_queried_at: Some(now),
            cold_refresh_count: 0,
        }
    }

    pub(in crate::contexts::daemon) fn output(mut self, s: String) -> Self {
        self.output = s;
        self
    }

    pub(in crate::contexts::daemon) fn pr_outputs(mut self, p: Vec<PrOutput>) -> Self {
        self.pr_outputs = p;
        self
    }

    pub(in crate::contexts::daemon) fn refresh_mode(mut self, m: RefreshMode) -> Self {
        self.refresh_mode = m;
        self
    }

    pub(in crate::contexts::daemon) fn last_queried_at(mut self, t: Option<Instant>) -> Self {
        self.last_queried_at = t;
        self
    }

    pub(in crate::contexts::daemon) fn cold_refresh_count(mut self, n: u32) -> Self {
        self.cold_refresh_count = n;
        self
    }

    pub(in crate::contexts::daemon) fn cwd(mut self, c: PathBuf) -> Self {
        self.cwd = c;
        self
    }

    pub(in crate::contexts::daemon) fn build(self) -> CacheEntryState {
        CacheEntryState {
            output: self.output,
            pr_outputs: self.pr_outputs,
            fetch_state: self.fetch_state,
            fetched_at: self.fetched_at,
            fetched_at_wall: self.fetched_at_wall,
            refresh_started_at: self.refresh_started_at,
            cwd: self.cwd,
            branch: self.branch,
            refresh_mode: self.refresh_mode,
            last_queried_at: self.last_queried_at,
            cold_refresh_count: self.cold_refresh_count,
        }
    }
}
