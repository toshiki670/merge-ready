use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use rustix::process::{Pid, Signal, kill_process, test_kill_process};

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

/// u32 の PID を rustix の `Pid` に変換する。
///
/// 0 や `i32` の範囲を超える値は実在しない PID なので `None` を返す。
fn to_pid(pid: u32) -> Option<Pid> {
    i32::try_from(pid).ok().and_then(Pid::from_raw)
}

/// プロセスが生存しているか調べる（`kill(pid, 0)` 相当）。
///
/// EPERM（プロセスは存在するが権限が無い）は「生存」とみなす。
/// ESRCH 等はプロセス不在として `false` を返す。
#[must_use]
pub fn is_alive(pid: u32) -> bool {
    let Some(pid) = to_pid(pid) else {
        return false;
    };
    match test_kill_process(pid) {
        Ok(()) => true,
        Err(e) => e == rustix::io::Errno::PERM,
    }
}

/// プロセスに SIGTERM を送る。送信できれば `true`。
pub fn terminate(pid: u32) -> bool {
    let Some(pid) = to_pid(pid) else {
        return false;
    };
    kill_process(pid, Signal::TERM).is_ok()
}

pub async fn wait_until_gone(pid: u32, path: &Path, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if !is_alive(pid) {
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

    #[test]
    fn is_alive_true_for_current_process() {
        assert!(super::is_alive(std::process::id()));
    }

    #[test]
    fn is_alive_false_for_zero_pid() {
        assert!(!super::is_alive(0));
    }

    #[test]
    fn is_alive_false_after_child_reaped() {
        let mut child = std::process::Command::new("/bin/sh")
            .args(["-c", "exit 0"])
            .spawn()
            .expect("spawn child");
        let pid = child.id();
        child.wait().expect("reap child");
        assert!(!super::is_alive(pid));
    }

    #[test]
    fn terminate_stops_a_running_child() {
        let mut child = std::process::Command::new("/bin/sh")
            .args(["-c", "sleep 30"])
            .spawn()
            .expect("spawn child");
        let pid = child.id();
        assert!(super::is_alive(pid));
        assert!(super::terminate(pid));
        child.wait().expect("reap child");
        assert!(!super::is_alive(pid));
    }

    #[test]
    fn terminate_false_for_zero_pid() {
        assert!(!super::terminate(0));
    }
}
