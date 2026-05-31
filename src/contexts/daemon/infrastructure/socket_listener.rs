use std::time::Duration;

use tokio::net::UnixListener;

use super::{paths::Paths, pid};
use crate::contexts::daemon::domain::daemon::DaemonError;

const BIND_RETRY_INTERVAL_MS: u64 = 100;
const BIND_RETRY_MAX: usize = 10;

pub(super) async fn bind(paths: &Paths) -> Result<UnixListener, DaemonError> {
    bind_with(
        paths,
        BIND_RETRY_MAX,
        Duration::from_millis(BIND_RETRY_INTERVAL_MS),
    )
    .await
}

async fn bind_with(
    paths: &Paths,
    retry_max: usize,
    retry_interval: Duration,
) -> Result<UnixListener, DaemonError> {
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
        Some(p) if pid::is_alive(p).await => {
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

    let listener = bind_with_retry(
        || UnixListener::bind(&socket_path),
        retry_max,
        retry_interval,
    )
    .await?;
    pid::write(std::process::id(), &pid_path);
    Ok(listener)
}

async fn bind_with_retry<F>(
    mut try_bind: F,
    retry_max: usize,
    retry_interval: Duration,
) -> Result<UnixListener, DaemonError>
where
    F: FnMut() -> std::io::Result<UnixListener>,
{
    let mut retries = 0;
    loop {
        match try_bind() {
            Ok(l) => return Ok(l),
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                if retries >= retry_max {
                    log::error!("socket already in use after retries, giving up");
                    return Err(DaemonError::AlreadyRunning);
                }
                retries += 1;
                tokio::time::sleep(retry_interval).await;
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
    use super::*;
    use std::assert_matches;
    use std::io::{Error, ErrorKind};
    use tempfile::tempdir;

    #[tokio::test]
    async fn bind_returns_failure_when_lock_file_cannot_be_opened() {
        // base_dir 自体が存在しない場合、`OpenOptions::create(true).open(lock_path)` は
        // 親ディレクトリ不在で ENOENT になり、open() の map_err 経路を踏む。
        let tmp = tempdir().unwrap();
        let bogus = tmp.path().join("does-not-exist-dir");
        let paths = Paths::new(bogus);

        let err = bind(&paths).await.unwrap_err();
        assert_matches!(err, DaemonError::Failure);
    }

    #[tokio::test]
    async fn bind_with_retry_returns_failure_on_non_addr_in_use_error() {
        let err = bind_with_retry(
            || -> std::io::Result<UnixListener> {
                Err(Error::new(ErrorKind::PermissionDenied, "denied"))
            },
            3,
            Duration::ZERO,
        )
        .await
        .unwrap_err();
        assert_matches!(err, DaemonError::Failure);
    }

    #[tokio::test]
    async fn bind_with_retry_returns_already_running_after_exhausting_retries() {
        let mut calls = 0_usize;
        let err = bind_with_retry(
            || -> std::io::Result<UnixListener> {
                calls += 1;
                Err(Error::new(ErrorKind::AddrInUse, "addr in use"))
            },
            2,
            Duration::ZERO,
        )
        .await
        .unwrap_err();
        assert_matches!(err, DaemonError::AlreadyRunning);
        // retry_max=2 のため、初回 + 2 リトライ = 計 3 回試行してから上限到達。
        assert_eq!(calls, 3);
    }
}
