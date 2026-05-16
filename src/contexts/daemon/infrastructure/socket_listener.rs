use std::os::unix::net::UnixListener;
use std::time::Duration;

use super::{paths::Paths, pid};
use crate::contexts::daemon::domain::daemon::DaemonError;

const BIND_RETRY_INTERVAL_MS: u64 = 100;
const BIND_RETRY_MAX: usize = 10;

pub(super) fn bind(paths: &Paths) -> Result<UnixListener, DaemonError> {
    let socket_path = paths.socket_path();
    let startup_lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(paths.lock_path())
        .map_err(|e| {
            log::error!("failed to open daemon lock file: {e}");
            eprintln!("merge-ready daemon: failed to open lock file: {e}");
            DaemonError::Failure
        })?;
    // `File::lock()` はブロッキングのため、ロック競合では Err にならず待機する。
    // Err になるのは flock 非対応 FS / ENOLCK / EIO 等 OS・FS レベル異常時のみで、
    // 通常のテスト環境では再現手段が無いため、この map_err 経路は意図的に未カバー。
    // `try_lock()` に変えると起動直列化の意図が壊れるため変更しない。
    startup_lock.lock().map_err(|e| {
        log::error!("failed to lock daemon startup: {e}");
        eprintln!("merge-ready daemon: failed to acquire startup lock: {e}");
        DaemonError::Failure
    })?;

    let pid_path = paths.pid_path();
    match pid::read(&pid_path) {
        Some(p) if pid::is_alive(p) => {
            log::error!("daemon is already running (pid {p})");
            eprintln!("merge-ready daemon is already running (pid {p})");
            return Err(DaemonError::AlreadyRunning);
        }
        Some(_) => {
            pid::remove(&pid_path);
            let _ = std::fs::remove_file(&socket_path);
        }
        None => {
            let _ = std::fs::remove_file(&socket_path);
        }
    }

    let mut retries = 0;
    loop {
        match UnixListener::bind(&socket_path) {
            Ok(l) => {
                pid::write(std::process::id(), &pid_path);
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
