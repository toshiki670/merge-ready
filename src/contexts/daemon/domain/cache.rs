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

// daemon::domain 内で共有する純粋述語ヘルパ（`RefreshPolicy` から呼ぶ）。
pub(in crate::contexts::daemon::domain) use transition::{has_recent_query, is_cold};
