use std::path::Path;
use std::time::Duration;

use crate::contexts::daemon::application::port::{EntryView, WatchPort};
use crate::contexts::daemon::domain::daemon::{DaemonError, DaemonLifecyclePort, DaemonStatus};

use super::daemon_server::RefreshFn;
use super::paths::Paths;
use super::{daemon_client::DaemonClient, daemon_server, pid};

const STOP_TIMEOUT: Duration = Duration::from_secs(2);

pub struct DaemonLifecycle {
    on_refresh: RefreshFn,
    paths: Paths,
}

impl DaemonLifecycle {
    pub fn new(on_refresh: RefreshFn) -> Self {
        Self {
            on_refresh,
            paths: Paths::default(),
        }
    }

    #[cfg(test)]
    pub fn with_paths(on_refresh: RefreshFn, paths: Paths) -> Self {
        Self { on_refresh, paths }
    }
}

impl DaemonLifecyclePort for DaemonLifecycle {
    async fn start(&self) -> Result<(), DaemonError> {
        daemon_server::run(self.on_refresh, self.paths.clone()).await
    }

    async fn stop(&self) -> bool {
        let pid_path = self.paths.pid_path();
        let client = DaemonClient::new(self.paths.socket_path());
        let running_pid = match pid::read(&pid_path) {
            Some(p) if pid::is_alive(p) => Some(p),
            _ => None,
        };
        if client.stop().await {
            return match running_pid {
                None => true,
                Some(p) => wait_or_terminate(p, &pid_path).await,
            };
        }
        let Some(p) = running_pid.or_else(|| pid::read(&pid_path)) else {
            return false;
        };
        if !pid::is_alive(p) {
            pid::remove(&pid_path);
            return false;
        }
        terminate_and_wait(p, &pid_path).await
    }

    async fn get_status(&self) -> Option<DaemonStatus> {
        DaemonClient::new(self.paths.socket_path())
            .status_raw()
            .await
            .map(|(entries, uptime_secs, version)| DaemonStatus {
                entries,
                uptime_secs,
                version,
            })
    }

    async fn get_pid(&self) -> Option<u32> {
        match pid::read(&self.paths.pid_path()) {
            Some(p) if pid::is_alive(p) => Some(p),
            _ => None,
        }
    }
}

async fn wait_or_terminate(p: u32, pid_path: &Path) -> bool {
    if pid::wait_until_gone(p, pid_path, STOP_TIMEOUT).await {
        return true;
    }
    terminate_and_wait(p, pid_path).await
}

async fn terminate_and_wait(p: u32, pid_path: &Path) -> bool {
    // ソケット経由が失敗した、または終了が遅い場合は SIGTERM でフォールバックする。
    pid::terminate(p) && pid::wait_until_gone(p, pid_path, STOP_TIMEOUT).await
}

impl WatchPort for DaemonLifecycle {
    async fn entries(&self) -> Option<Vec<EntryView>> {
        DaemonClient::new(self.paths.socket_path())
            .entries_raw()
            .await
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
    use std::future::Future;
    use std::path::PathBuf;
    use std::pin::Pin;

    use tempfile::tempdir;

    use super::*;
    use crate::contexts::daemon::domain::cache::RepoId;

    fn noop_refresh(
        _repo_id: RepoId,
        _cwd: PathBuf,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'static>> {
        Box::pin(async move {})
    }

    fn noop_lifecycle(paths: Paths) -> DaemonLifecycle {
        DaemonLifecycle::with_paths(noop_refresh, paths)
    }

    #[tokio::test]
    async fn get_pid_returns_none_when_no_pid_file() {
        let dir = tempdir().unwrap();
        let lifecycle = noop_lifecycle(Paths::new(dir.path().to_path_buf()));
        assert_eq!(lifecycle.get_pid().await, None);
    }

    #[tokio::test]
    async fn get_pid_returns_some_when_pid_is_alive() {
        let dir = tempdir().unwrap();
        let paths = Paths::new(dir.path().to_path_buf());
        pid::write(std::process::id(), &paths.pid_path());
        let lifecycle = noop_lifecycle(paths);
        assert_eq!(lifecycle.get_pid().await, Some(std::process::id()));
    }

    #[tokio::test]
    async fn stop_returns_false_when_daemon_not_running() {
        let dir = tempdir().unwrap();
        let lifecycle = noop_lifecycle(Paths::new(dir.path().to_path_buf()));
        assert!(!lifecycle.stop().await);
    }

    #[tokio::test]
    async fn get_status_returns_none_when_daemon_not_running() {
        let dir = tempdir().unwrap();
        let lifecycle = noop_lifecycle(Paths::new(dir.path().to_path_buf()));
        assert!(lifecycle.get_status().await.is_none());
    }
}
