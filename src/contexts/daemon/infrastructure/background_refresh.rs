use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime};

use super::server_state::DaemonState;
use crate::contexts::daemon::domain::cache::{
    Effect, RepoId, SchedulerTickInput, on_scheduler_tick,
};

pub(super) fn collect_targets(state: &Arc<Mutex<DaemonState>>) -> Vec<(RepoId, PathBuf)> {
    let mut s = state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let config = s.config;
    let policy = config.policy;

    // rate_limit_aware が OFF のとき snapshot を強制的に None として扱う必要がある。
    // 簡単のため、snapshot 参照を作らず、policy 経由で扱う代わりに、
    // on_scheduler_tick の入力では snapshot を直接渡せないので、
    // OFF のとき CacheStore::latest_rate_limit() を一時的に隠す方法はない。
    // ここでは config.rate_limit_aware を見て、tick 関数の入力から外す。
    // Step 5/6 でこの分岐を SchedulerTickInput 自体に持たせるか整理する。
    // 暫定: tick 関数内では常に store.latest_rate_limit() を使う。
    // rate_limit_aware=false のときは latest_rate_limit を None に保つ運用で対応する
    // （rate_limit fetcher スレッド自体が起動しないので latest_rate_limit は永遠に None）。
    let input = SchedulerTickInput {
        now: Instant::now(),
        now_wall: SystemTime::now(),
        policy: &policy,
        stale_ttl: config.stale_ttl_secs,
        refresh_lock_timeout_secs: config.refresh_lock_timeout_secs,
        entry_max_age_secs: config.entry_max_age_secs,
    };

    let (new_store, effects) = on_scheduler_tick(&s.cache_store, input);
    s.cache_store = new_store;

    let mut targets = Vec::new();
    for e in effects {
        match e {
            Effect::SpawnRefresh { repo_id, cwd } => targets.push((repo_id, cwd)),
            Effect::RecordExpired { repo_id } => log::debug!("entry expired: {repo_id:?}"),
            Effect::EmitOutput(_) | Effect::EnterBackoff { .. } => {}
        }
    }
    targets
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contexts::daemon::domain::cache::CacheEntry;
    use crate::shared::protocol::PrOutput;
    use crate::shared::refresh_mode::RefreshMode;
    use std::time::Duration;

    fn entry_with_prs(pr_count: usize, active: bool) -> CacheEntry {
        let mut e = CacheEntry::new(PathBuf::from("/tmp"), "main".to_owned(), 5);
        let prs: Vec<PrOutput> = (0..pr_count)
            .map(|i| PrOutput {
                pr_id: i as u64,
                output: String::new(),
            })
            .collect();
        let mode = if active {
            RefreshMode::Warm
        } else {
            RefreshMode::Terminal
        };
        e.update("output".to_owned(), prs, mode);
        e
    }

    // ── collect_targets の backoff スキップ ───────────────────────────────────

    use super::super::server_config::DaemonServerConfig;
    use crate::contexts::daemon::domain::refresh_policy::RefreshPolicy;

    fn test_config() -> DaemonServerConfig {
        DaemonServerConfig {
            stale_ttl_secs: 5,
            refresh_lock_timeout_secs: 120,
            entry_max_age_secs: 60,
            scheduler_tick_secs: 2,
            socket_check_interval_secs: 5,
            policy: RefreshPolicy {
                hot_recent_query_secs: 30,
                hot_with_query_secs: 2,
                hot_without_query_secs: 10,
                warm_refresh_secs: 180,
                warm_to_cold_secs: 1800,
                cold_early_secs: 1800,
                cold_late_secs: 3600,
                cold_early_limit: 10,
            },
            rate_limit_aware: true,
            rate_limit_fetch_interval_secs: 60,
        }
    }

    #[test]
    fn collect_targets_returns_empty_when_in_backoff() {
        let mut state = DaemonState::new(test_config());
        state
            .cache_store
            .entries_mut()
            .insert(RepoId::new("test".to_owned()), entry_with_prs(1, true));
        state
            .cache_store
            .set_backoff(Instant::now() + Duration::from_mins(1));
        let state = Arc::new(Mutex::new(state));
        let targets = collect_targets(&state);
        assert!(targets.is_empty());
    }

    #[test]
    fn collect_targets_clears_expired_backoff() {
        let mut state = DaemonState::new(test_config());
        let past = Instant::now()
            .checked_sub(Duration::from_secs(10))
            .expect("past");
        state.cache_store.set_backoff(past);
        let state = Arc::new(Mutex::new(state));
        let _ = collect_targets(&state);
        let s = state.lock().unwrap();
        assert!(
            s.cache_store.backoff_until().is_none(),
            "expired backoff should be cleared"
        );
    }
}
