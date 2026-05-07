use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use super::server_state::DaemonState;
use crate::contexts::daemon::domain::cache::{RefreshMode, RepoId};

pub(super) fn collect_targets(state: &Arc<Mutex<DaemonState>>) -> Vec<(RepoId, PathBuf)> {
    let mut s = state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let config = s.config;
    let policy = config.policy;

    s.entries
        .retain(|_, entry| !entry.is_expired(config.entry_max_age_secs));

    let mut targets = Vec::new();
    for (repo_id, entry) in &mut s.entries {
        if !entry.is_active() {
            continue;
        }
        if entry.is_refreshing() && entry.refresh_lock_expired(config.refresh_lock_timeout_secs) {
            entry.clear_refresh_lock();
        }
        if entry.is_refreshing() {
            continue;
        }
        let interval = policy.effective_refresh_interval_secs(entry);
        if entry.fetched_at.elapsed().as_secs() < interval {
            continue;
        }
        if entry.refresh_mode() == RefreshMode::Warm && entry.is_cold(policy.warm_to_cold_secs) {
            entry.increment_cold_count();
        }
        entry.mark_refreshing();
        targets.push((repo_id.clone(), entry.cwd.clone()));
    }

    targets
}

#[cfg(test)]
mod tests {
    use super::super::server_config::DaemonServerConfig;
    use super::*;
    use crate::contexts::daemon::domain::cache::{CacheEntry, RefreshMode};
    use std::time::{Duration, Instant};

    fn state() -> Arc<Mutex<DaemonState>> {
        Arc::new(Mutex::new(DaemonState::new(DaemonServerConfig::from_env())))
    }

    fn make_entry(output: &str, refresh_mode: RefreshMode) -> CacheEntry {
        let mut e = CacheEntry::new(PathBuf::new(), String::new(), 5);
        e.update(output.to_owned(), vec![], refresh_mode);
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

    #[test]
    fn background_refresh_skips_terminal_entry() {
        let state = state();
        {
            let mut s = state.lock().unwrap();
            s.entries.insert(
                RepoId::new("repo"),
                make_stale_entry("✓ Ready for merge", RefreshMode::Terminal, 9999),
            );
        }
        let targets = collect_targets(&state);
        assert!(targets.is_empty());
    }

    #[test]
    fn background_refresh_includes_stale_hot_entry() {
        let state = state();
        {
            let mut s = state.lock().unwrap();
            let mut entry = make_stale_entry("⧖ Wait for CI", RefreshMode::Hot, 9999);
            entry.cwd = PathBuf::from("/some/repo");
            s.entries.insert(RepoId::new("repo"), entry);
        }
        let targets = collect_targets(&state);
        assert_eq!(targets.len(), 1);
    }

    #[test]
    fn background_refresh_increments_cold_count() {
        let repo_id = RepoId::new("repo");
        let state = state();
        {
            let mut s = state.lock().unwrap();
            let mut entry = make_stale_entry("✓ Ready for merge", RefreshMode::Warm, 9999);
            entry.last_queried_at = Some(
                Instant::now()
                    .checked_sub(Duration::from_secs(s.config.policy.warm_to_cold_secs + 1))
                    .unwrap(),
            );
            entry.cold_refresh_count = 3;
            entry.cwd = PathBuf::from("/some/repo");
            s.entries.insert(repo_id.clone(), entry);
        }
        collect_targets(&state);
        let s = state.lock().unwrap();
        assert_eq!(s.entries[&repo_id].cold_refresh_count(), 4);
    }

    #[test]
    fn background_refresh_removes_expired_entries() {
        let state = state();
        {
            let mut s = state.lock().unwrap();
            let mut entry = make_entry("✓ Ready for merge", RefreshMode::Warm);
            entry.last_queried_at = Some(
                Instant::now()
                    .checked_sub(Duration::from_secs(s.config.entry_max_age_secs + 1))
                    .unwrap(),
            );
            s.entries.insert(RepoId::new("repo"), entry);
        }
        collect_targets(&state);
        let s = state.lock().unwrap();
        assert!(s.entries.is_empty());
    }
}
