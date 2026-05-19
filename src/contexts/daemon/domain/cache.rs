//! daemon キャッシュドメイン。
//!
//! 状態（`CacheStore` / `CacheEntryState`）と純粋な状態遷移関数
//! (`on_query` / `on_refresh_completed` / `on_scheduler_tick` /
//! `on_rate_limit_observed`) を提供する。
//!
//! daemon の各 edge (`request_handler` / `background_refresh` /
//! `daemon_server::update_state_from_snapshot`) は `Mutex<DaemonState>` を
//! ロックして純粋関数を呼び、戻り値の `(CacheStore, Vec<Effect>)` を反映する。
//! ドメイン層から `Instant::now()` / `SystemTime::now()` を呼ばないので、
//! 状態遷移は完全に決定的（仮想時刻でテスト可能）。

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
