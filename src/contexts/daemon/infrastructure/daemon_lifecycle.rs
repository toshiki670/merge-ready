use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use crate::contexts::daemon::application::port::{EntryView, WatchPort};
use crate::contexts::daemon::domain::cache::RepoId;
use crate::contexts::daemon::domain::daemon::{DaemonError, DaemonLifecyclePort, DaemonStatus};

use super::paths::Paths;
use super::{daemon_client::DaemonClient, daemon_server, pid};

type RefreshCallback = dyn Fn(&RepoId, &std::path::Path) + Send + Sync + 'static;
const STOP_TIMEOUT: Duration = Duration::from_secs(2);

pub struct DaemonLifecycle {
    on_refresh: Arc<RefreshCallback>,
    paths: Paths,
}

impl DaemonLifecycle {
    pub fn new(on_refresh: impl Fn(&RepoId, &std::path::Path) + Send + Sync + 'static) -> Self {
        Self {
            on_refresh: Arc::new(on_refresh),
            paths: Paths::default(),
        }
    }

    #[cfg(test)]
    pub fn with_paths(
        on_refresh: impl Fn(&RepoId, &std::path::Path) + Send + Sync + 'static,
        paths: Paths,
    ) -> Self {
        Self {
            on_refresh: Arc::new(on_refresh),
            paths,
        }
    }
}

impl DaemonLifecyclePort for DaemonLifecycle {
    fn start(&self) -> Result<(), DaemonError> {
        daemon_server::run(&self.on_refresh, self.paths.clone())
    }

    fn stop(&self) -> bool {
        let pid_path = self.paths.pid_path();
        let client = DaemonClient::new(self.paths.socket_path());
        let running_pid = pid::read(&pid_path).filter(|&p| pid::is_alive(p));
        if client.stop() {
            return running_pid.is_none_or(|p| wait_or_terminate(p, &pid_path));
        }
        let Some(p) = running_pid.or_else(|| pid::read(&pid_path)) else {
            return false;
        };
        if !pid::is_alive(p) {
            pid::remove(&pid_path);
            return false;
        }
        terminate_and_wait(p, &pid_path)
    }

    fn get_status(&self) -> Option<DaemonStatus> {
        DaemonClient::new(self.paths.socket_path())
            .status_raw()
            .map(|(entries, uptime_secs, version)| DaemonStatus {
                entries,
                uptime_secs,
                version,
            })
    }

    fn get_pid(&self) -> Option<u32> {
        pid::read(&self.paths.pid_path()).filter(|&p| pid::is_alive(p))
    }
}

fn wait_or_terminate(p: u32, pid_path: &Path) -> bool {
    if pid::wait_until_gone(p, pid_path, STOP_TIMEOUT) {
        return true;
    }
    terminate_and_wait(p, pid_path)
}

fn terminate_and_wait(p: u32, pid_path: &Path) -> bool {
    // ソケット経由が失敗した、または終了が遅い場合は SIGTERM でフォールバックする。
    let signalled = std::process::Command::new("kill")
        .args(["-TERM", &p.to_string()])
        .status()
        .is_ok_and(|s| s.success());
    signalled && pid::wait_until_gone(p, pid_path, STOP_TIMEOUT)
}

impl WatchPort for DaemonLifecycle {
    fn entries(&self) -> Option<Vec<EntryView>> {
        DaemonClient::new(self.paths.socket_path())
            .entries_raw()
            .map(|dtos| {
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
    use tempfile::tempdir;

    use super::*;

    fn noop_lifecycle(paths: Paths) -> DaemonLifecycle {
        DaemonLifecycle::with_paths(|_, _| {}, paths)
    }

    #[test]
    fn get_pid_returns_none_when_no_pid_file() {
        let dir = tempdir().unwrap();
        let lifecycle = noop_lifecycle(Paths::new(dir.path().to_path_buf()));
        assert_eq!(lifecycle.get_pid(), None);
    }

    #[test]
    fn get_pid_returns_none_when_pid_is_not_alive() {
        let dir = tempdir().unwrap();
        let paths = Paths::new(dir.path().to_path_buf());
        pid::write(u32::MAX, &paths.pid_path());
        let lifecycle = noop_lifecycle(paths);
        assert_eq!(lifecycle.get_pid(), None);
    }

    #[test]
    fn stop_returns_false_when_daemon_not_running() {
        let dir = tempdir().unwrap();
        let lifecycle = noop_lifecycle(Paths::new(dir.path().to_path_buf()));
        assert!(!lifecycle.stop());
    }

    #[test]
    fn get_status_returns_none_when_daemon_not_running() {
        let dir = tempdir().unwrap();
        let lifecycle = noop_lifecycle(Paths::new(dir.path().to_path_buf()));
        assert!(lifecycle.get_status().is_none());
    }
}
