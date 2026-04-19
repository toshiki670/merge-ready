use std::sync::Arc;

use crate::contexts::status_cache::domain::daemon::{DaemonLifecyclePort, DaemonStatus};

use super::{daemon_client::DaemonClient, daemon_server, pid};

pub struct DaemonLifecycle {
    on_refresh: Arc<dyn Fn(&str) + Send + Sync + 'static>,
}

impl DaemonLifecycle {
    pub fn new(on_refresh: impl Fn(&str) + Send + Sync + 'static) -> Self {
        Self {
            on_refresh: Arc::new(on_refresh),
        }
    }
}

impl DaemonLifecyclePort for DaemonLifecycle {
    fn start(&self) {
        if let Some(p) = pid::read() {
            if pid::is_alive(p) {
                eprintln!("merge-ready daemon is already running (pid {p})");
                std::process::exit(1);
            }
            pid::remove();
        }
        daemon_server::run(&self.on_refresh);
    }

    fn stop(&self) -> bool {
        if DaemonClient::stop() {
            return true;
        }
        let Some(p) = pid::read() else {
            return false;
        };
        if !pid::is_alive(p) {
            pid::remove();
            return false;
        }
        // ソケット経由が失敗した場合は SIGTERM でフォールバック
        std::process::Command::new("kill")
            .args(["-TERM", &p.to_string()])
            .status()
            .is_ok_and(|s| s.success())
    }

    fn get_status(&self) -> Option<DaemonStatus> {
        DaemonClient::status_raw().map(|(pid, entries, uptime_secs)| DaemonStatus {
            pid,
            entries,
            uptime_secs,
        })
    }
}
