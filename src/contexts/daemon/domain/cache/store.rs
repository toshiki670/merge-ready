use std::collections::HashMap;
use std::time::Instant;

use super::entry::CacheEntryState;
use super::repo_id::RepoId;
use crate::contexts::daemon::domain::rate_limit_snapshot::RateLimitSnapshot;

/// daemon が保持する状態を純粋値オブジェクトとして集約したもの。
///
/// `transition` モジュールの純粋関数だけが新しい `CacheStore` を構築する。
/// edge (`request_handler` / `background_refresh` / `daemon_server`) は
/// `Mutex` のロック取得後にこの値を読み・新しい値で差し替える。
#[derive(Debug, Clone, Default)]
pub struct CacheStore {
    entries: HashMap<RepoId, CacheEntryState>,
    latest_rate_limit: Option<RateLimitSnapshot>,
    backoff_until: Option<Instant>,
}

impl CacheStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn entries(&self) -> &HashMap<RepoId, CacheEntryState> {
        &self.entries
    }

    pub fn latest_rate_limit(&self) -> Option<&RateLimitSnapshot> {
        self.latest_rate_limit.as_ref()
    }

    pub fn backoff_until(&self) -> Option<Instant> {
        self.backoff_until
    }

    /// `now` 時点で backoff が有効か。
    pub fn is_backed_off(&self, now: Instant) -> bool {
        self.backoff_until.is_some_and(|t| now < t)
    }

    // ── 構築 API (transition モジュール内部から呼ばれる) ─────────

    pub(super) fn with_entries(mut self, entries: HashMap<RepoId, CacheEntryState>) -> Self {
        self.entries = entries;
        self
    }

    pub(super) fn with_backoff_until(mut self, t: Option<Instant>) -> Self {
        self.backoff_until = t;
        self
    }

    // ───────────────────────────────────────────────────────────────
    // 段階移行用の mutating API（Issue #339 最終 Phase で削除予定）
    //
    // `collect_targets` / `update_state_from_snapshot` を transition 経由に
    // 切り替えるまでの暫定。`pub(in crate::contexts::daemon)` で daemon
    // コンテキスト内部に閉じ込め、外部レイヤへの leak を防ぐ。
    // ───────────────────────────────────────────────────────────────

    pub(in crate::contexts::daemon) fn entries_mut(
        &mut self,
    ) -> &mut HashMap<RepoId, CacheEntryState> {
        &mut self.entries
    }

    pub(in crate::contexts::daemon) fn set_latest_rate_limit(
        &mut self,
        r: Option<RateLimitSnapshot>,
    ) {
        self.latest_rate_limit = r;
    }

    pub(in crate::contexts::daemon) fn set_backoff(&mut self, t: Instant) {
        self.backoff_until = Some(t);
    }

    pub(in crate::contexts::daemon) fn clear_backoff_if_expired(&mut self, now: Instant) {
        if self.backoff_until.is_some_and(|t| now >= t) {
            self.backoff_until = None;
        }
    }

    pub(in crate::contexts::daemon) fn should_backoff(&self, now: Instant) -> bool {
        self.is_backed_off(now)
    }
}
