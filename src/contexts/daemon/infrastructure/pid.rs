use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

pub fn write(pid: u32, path: &Path) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, pid.to_string());
}

#[must_use]
pub fn read(path: &Path) -> Option<u32> {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| s.trim().parse().ok())
}

pub fn remove(path: &Path) {
    let _ = fs::remove_file(path);
}

pub async fn is_alive(pid: u32) -> bool {
    tokio::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .is_ok_and(|s| s.success())
}

pub async fn wait_until_gone(pid: u32, path: &Path, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if !is_alive(pid).await {
            remove(path);
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    #[test]
    fn write_and_read_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("daemon.pid");
        super::write(42, &path);
        assert_eq!(super::read(&path), Some(42));
    }

    #[test]
    fn remove_deletes_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("daemon.pid");
        super::write(1, &path);
        assert!(path.exists());
        super::remove(&path);
        assert!(!path.exists());
    }

    #[test]
    fn read_returns_none_when_missing() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("daemon.pid");
        assert_eq!(super::read(&path), None);
    }
}
