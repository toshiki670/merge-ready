mod effect;
mod entry;
mod event;
mod port;
mod repo_id;
mod store;
mod transition;

pub use effect::Effect;
pub use entry::CacheEntryState;
pub use event::{QueryEvent, RateLimitObservedEvent, RefreshCompletedEvent, SchedulerTickInput};
pub use port::CachePort;
pub use repo_id::RepoId;
pub use store::CacheStore;
pub use transition::{on_query, on_rate_limit_observed, on_refresh_completed, on_scheduler_tick};

// daemon::domain 内で共有する純粋述語ヘルパ（`RefreshPolicy` と `transition` で共有）。
pub(in crate::contexts::daemon::domain) use transition::{
    fetched_at_elapsed, has_recent_query, is_cold, is_cold_or_never_queried, is_expired, is_fresh,
    refresh_lock_expired,
};

/// 旧名互換 alias。`CacheEntryState` への段階移行のために残す。
/// Issue #339 の最終 Phase で削除する。
pub type CacheEntry = CacheEntryState;
