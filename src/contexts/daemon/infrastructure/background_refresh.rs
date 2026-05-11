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
