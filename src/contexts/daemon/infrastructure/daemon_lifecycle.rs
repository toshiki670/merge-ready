use std::process::ExitCode;
use std::sync::Arc;

use crate::contexts::daemon::domain::daemon::{DaemonLifecyclePort, DaemonStatus};

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
    fn start(&self) -> ExitCode {
        if let Some(p) = pid::read() {
            if pid::is_alive(p) {
                log::error!("daemon is already running (pid {p})");
                eprintln!("merge-ready daemon is already running (pid {p})");
                return ExitCode::FAILURE;
            }
            pid::remove();
        }
        daemon_server::run(&self.on_refresh)
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
        DaemonClient::status_raw().map(|(pid, entries, uptime_secs, version)| DaemonStatus {
            pid,
            entries,
            uptime_secs,
            version,
        })
    }
}
