use crate::contexts::status_cache::domain::{DaemonLifecyclePort, DaemonStatus};

use super::{daemon_client::DaemonClient, daemon_server, pid};

pub struct DaemonLifecycle;

impl DaemonLifecyclePort for DaemonLifecycle {
    fn start(&self) {
        if let Some(p) = pid::read() {
            if pid::is_alive(p) {
                eprintln!("merge-ready daemon is already running (pid {p})");
                std::process::exit(1);
            }
            pid::remove();
        }
        daemon_server::run();
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
