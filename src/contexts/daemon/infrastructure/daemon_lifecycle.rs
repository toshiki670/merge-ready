use std::sync::Arc;
use std::time::Duration;

use crate::contexts::daemon::application::port::{EntryView, WatchPort};
use crate::contexts::daemon::domain::cache::RepoId;
use crate::contexts::daemon::domain::daemon::{DaemonError, DaemonLifecyclePort, DaemonStatus};

use super::{daemon_client::DaemonClient, daemon_server, pid};

type RefreshCallback = dyn Fn(&RepoId, &std::path::Path) + Send + Sync + 'static;
#[cfg(not(test))]
const STOP_TIMEOUT: Duration = Duration::from_secs(2);
#[cfg(test)]
const STOP_TIMEOUT: Duration = Duration::from_millis(50);

pub struct DaemonLifecycle {
    on_refresh: Arc<RefreshCallback>,
}

impl DaemonLifecycle {
    pub fn new(on_refresh: impl Fn(&RepoId, &std::path::Path) + Send + Sync + 'static) -> Self {
        Self {
            on_refresh: Arc::new(on_refresh),
        }
    }
}

impl DaemonLifecyclePort for DaemonLifecycle {
    fn start(&self) -> Result<(), DaemonError> {
        daemon_server::run(&self.on_refresh)
    }

    fn stop(&self) -> bool {
        let running_pid = pid::read().filter(|&p| pid::is_alive(p));
        if DaemonClient::stop() {
            return running_pid.is_none_or(wait_or_terminate);
        }
        let Some(p) = running_pid.or_else(pid::read) else {
            return false;
        };
        if !pid::is_alive(p) {
            pid::remove();
            return false;
        }
        terminate_and_wait(p)
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

fn wait_or_terminate(p: u32) -> bool {
    if pid::wait_until_gone(p, STOP_TIMEOUT) {
        return true;
    }
    terminate_and_wait(p)
}

fn terminate_and_wait(p: u32) -> bool {
    // ソケット経由が失敗した、または終了が遅い場合は SIGTERM でフォールバックする。
    let signalled = std::process::Command::new("kill")
        .args(["-TERM", &p.to_string()])
        .status()
        .is_ok_and(|s| s.success());
    signalled && pid::wait_until_gone(p, STOP_TIMEOUT)
}

impl WatchPort for DaemonLifecycle {
    fn entries(&self) -> Option<Vec<EntryView>> {
        DaemonClient::entries_raw().map(|dtos| {
            dtos.into_iter()
                .map(|dto| EntryView {
                    cwd: dto.cwd,
                    branch: dto.branch,
                    pr_id: dto.pr_id,
                    output: dto.output,
                    cached_at_secs: dto.cached_at_secs,
                })
                .collect()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{terminate_and_wait, wait_or_terminate};

    #[test]
    fn terminate_and_wait_nonexistent_pid_returns_false() {
        // kill -TERM u32::MAX → fails (no such process) → signalled=false → false immediately
        assert!(!terminate_and_wait(u32::MAX));
    }

    #[test]
    fn wait_or_terminate_falls_back_to_sigterm_when_not_responding() {
        // Spawn a process that ignores SIGTERM.
        // wait_or_terminate → wait_until_gone times out (STOP_TIMEOUT=50ms in tests)
        // → terminate_and_wait sends SIGTERM (ignored) → wait_until_gone times out again → false.
        let mut child = std::process::Command::new("sh")
            .args(["-c", "trap '' TERM; sleep 100"])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("spawn sh");
        let pid = child.id();

        let result = wait_or_terminate(pid);
        assert!(
            !result,
            "SIGTERM-ignoring process should cause false return"
        );

        child.kill().ok();
        child.wait().ok();
    }
}
