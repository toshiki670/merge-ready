use std::os::unix::net::UnixListener;
use std::time::Duration;

use super::{paths, pid};
use crate::contexts::daemon::domain::daemon::DaemonError;

const BIND_RETRY_INTERVAL_MS: u64 = 100;
const BIND_RETRY_MAX: usize = 10;

pub(super) fn bind(socket_path: &std::path::Path) -> Result<UnixListener, DaemonError> {
    let startup_lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(paths::lock_path())
        .map_err(|e| {
            log::error!("failed to open daemon lock file: {e}");
            eprintln!("merge-ready daemon: failed to open lock file: {e}");
            DaemonError::Failure
        })?;
    startup_lock.lock().map_err(|e| {
        log::error!("failed to lock daemon startup: {e}");
        eprintln!("merge-ready daemon: failed to acquire startup lock: {e}");
        DaemonError::Failure
    })?;

    match pid::read() {
        Some(p) if pid::is_alive(p) => {
            log::error!("daemon is already running (pid {p})");
            eprintln!("merge-ready daemon is already running (pid {p})");
            return Err(DaemonError::AlreadyRunning);
        }
        Some(_) => {
            pid::remove();
            let _ = std::fs::remove_file(socket_path);
        }
        None => {
            let _ = std::fs::remove_file(socket_path);
        }
    }

    let mut retries = 0;
    loop {
        match UnixListener::bind(socket_path) {
            Ok(l) => {
                pid::write(std::process::id());
                return Ok(l);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                if retries >= BIND_RETRY_MAX {
                    log::error!("socket already in use after retries, giving up");
                    return Err(DaemonError::AlreadyRunning);
                }
                retries += 1;
                std::thread::sleep(Duration::from_millis(BIND_RETRY_INTERVAL_MS));
            }
            Err(e) => {
                log::error!("failed to bind socket: {e}");
                eprintln!("merge-ready daemon: failed to bind socket: {e}");
                return Err(DaemonError::Failure);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::contexts::daemon::domain::daemon::DaemonError;

    use super::{bind, paths, pid};

    #[test]
    fn bind_returns_failure_when_socket_path_exceeds_os_limit() {
        // Skip when a real daemon is already running: bind() reads the global PID file
        // and returns AlreadyRunning before reaching the socket-bind step (Issue #299).
        if pid::read().is_some_and(pid::is_alive) {
            return;
        }

        // Ensure the lock file's parent directory exists; bind() opens
        // paths::lock_path() (TMPDIR/merge-ready/daemon.lock) before binding the socket.
        let _ = std::fs::create_dir_all(paths::base_dir());

        // Unix socket paths are limited to ~104 bytes (macOS) / ~108 bytes (Linux).
        // A 205-char path exceeds both limits, causing UnixListener::bind to fail
        // with ENAMETOOLONG — not AddrInUse — which exercises lines 58–61.
        let socket_path = format!("/tmp/{}", "a".repeat(200));
        let result = bind(Path::new(&socket_path));
        assert!(
            matches!(result, Err(DaemonError::Failure)),
            "expected Failure for oversized path, got: {result:?}"
        );
    }
}
