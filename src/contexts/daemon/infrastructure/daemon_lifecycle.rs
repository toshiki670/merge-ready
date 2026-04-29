use std::sync::Arc;

use crate::contexts::daemon::domain::daemon::{DaemonError, DaemonLifecyclePort, DaemonStatus};

use super::{daemon_client::DaemonClient, daemon_server, pid};

type RefreshCallback = dyn Fn(&str, &std::path::Path) + Send + Sync + 'static;

pub struct DaemonLifecycle {
    on_refresh: Arc<RefreshCallback>,
}

impl DaemonLifecycle {
    pub fn new(on_refresh: impl Fn(&str, &std::path::Path) + Send + Sync + 'static) -> Self {
        Self {
            on_refresh: Arc::new(on_refresh),
        }
    }
}

impl DaemonLifecyclePort for DaemonLifecycle {
    fn start(&self) -> Result<(), DaemonError> {
        if let Some(p) = pid::read() {
            if pid::is_alive(p) {
                log::error!("daemon is already running (pid {p})");
                eprintln!("merge-ready daemon is already running (pid {p})");
                return Err(DaemonError::AlreadyRunning);
            }
            pid::remove();
        }
        daemon_server::run(&self.on_refresh).map_err(|()| DaemonError::Failure)
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
        DaemonClient::status_raw().map(|(entries, uptime_secs, version)| DaemonStatus {
            entries,
            uptime_secs,
            version,
        })
    }

    fn get_pid(&self) -> Option<u32> {
        pid::read().filter(|&p| pid::is_alive(p))
    }
}
