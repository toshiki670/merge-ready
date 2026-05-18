use std::path::PathBuf;
use std::time::{Instant, SystemTime};

use crate::contexts::daemon::domain::rate_limit_snapshot::RateLimitSnapshot;
use crate::contexts::daemon::domain::refresh_policy::RefreshPolicy;
use crate::shared::protocol::PrOutput;
use crate::shared::refresh_mode::RefreshMode;

/// Query リクエスト受付イベント。
pub struct QueryEvent {
    pub cwd: PathBuf,
    pub branch: String,
    pub now: Instant,
    pub now_wall: SystemTime,
    pub stale_ttl: u64,
}

/// バックグラウンドリフレッシュ完了イベント。
pub struct RefreshCompletedEvent {
    pub output: String,
    pub pr_outputs: Vec<PrOutput>,
    pub refresh_mode: RefreshMode,
    pub now: Instant,
    pub now_wall: SystemTime,
}

/// スケジューラ tick の入力。
pub struct SchedulerTickInput<'a> {
    pub now: Instant,
    pub now_wall: SystemTime,
    pub policy: &'a RefreshPolicy,
    pub stale_ttl: u64,
    pub refresh_lock_timeout_secs: u64,
    pub entry_max_age_secs: u64,
}

/// Rate limit スナップショット観測イベント。
pub struct RateLimitObservedEvent {
    pub snapshot: RateLimitSnapshot,
    pub now: Instant,
    pub now_wall: SystemTime,
}
