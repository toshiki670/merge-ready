use std::collections::HashMap;
use std::time::Instant;

use super::server_config::DaemonServerConfig;
use crate::contexts::daemon::domain::cache::{CacheEntry, RepoId};

pub(super) struct DaemonState {
    pub(super) entries: HashMap<RepoId, CacheEntry>,
    pub(super) started_at: Instant,
    pub(super) config: DaemonServerConfig,
}

impl DaemonState {
    pub(super) fn new(config: DaemonServerConfig) -> Self {
        Self {
            entries: HashMap::new(),
            started_at: Instant::now(),
            config,
        }
    }
}
