use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use super::{paths, pid};

const RESTART_GRACE_MS: u64 = 30;

pub(super) fn cleanup() {
    let _ = std::fs::remove_file(paths::socket_path());
    pid::remove();
}

pub(super) fn spawn_self_as_daemon() {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let _ = std::process::Command::new(&exe)
        .args(["daemon", "start"])
        .env(paths::DAEMON_INNER_ENV, "1")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

pub(super) fn restart_once(restart_started: &Arc<AtomicBool>, exit_tx: &mpsc::Sender<()>) {
    if restart_started
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        std::thread::sleep(Duration::from_millis(RESTART_GRACE_MS));
        cleanup();
        spawn_self_as_daemon();
        let _ = exit_tx.send(());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU32;

    #[test]
    fn restart_gate_allows_only_one_thread() {
        let restart_started = Arc::new(AtomicBool::new(false));
        let count = Arc::new(AtomicU32::new(0));
        let handles: Vec<_> = (0..10)
            .map(|_| {
                let rs = Arc::clone(&restart_started);
                let c = Arc::clone(&count);
                std::thread::spawn(move || {
                    if rs
                        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                        .is_ok()
                    {
                        c.fetch_add(1, Ordering::Relaxed);
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(count.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn restart_once_skips_when_already_started() {
        let restart_started = Arc::new(AtomicBool::new(true));
        let (tx, rx) = std::sync::mpsc::channel();
        restart_once(&restart_started, &tx);
        assert!(
            rx.try_recv().is_err(),
            "既に開始済みなら exit シグナルは送られないはず"
        );
    }
}
