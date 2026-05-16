//! daemon プロセスを管理するテストヘルパー。

use std::fs;
use std::io::Read;
use std::path::Path;

use super::{TestEnv, run_prompt_with_timeout};

/// daemon プロセスを管理するテストヘルパー。
///
/// `merge-ready daemon start` CLI の正常終了を起動完了シグナルとして使う。
/// CLI は内部 daemon の `"ready\n"` ハンドシェイクを読んだ後に exit するため、
/// pid と socket が両方書き込まれてから起動完了とみなせる。
/// Drop 時に daemon を停止する。
pub struct DaemonHandle {
    process: std::process::Child,
    pub(super) base_dir: std::path::PathBuf,
}

impl DaemonHandle {
    pub(super) fn new(process: std::process::Child, base_dir: std::path::PathBuf) -> Self {
        DaemonHandle { process, base_dir }
    }
}

const STOP_WAIT_MS: u64 = 2000;
const START_WAIT_MS: u64 = 5000;

impl DaemonHandle {
    /// daemon を起動し、CLI が exit するまで最大 5000ms 待機する。
    #[must_use]
    pub fn start(env: &TestEnv) -> Self {
        Self::start_with_env(env, &[])
    }

    /// 追加の環境変数を指定して daemon を起動する。
    #[must_use]
    pub fn start_with_env(env: &TestEnv, extra_envs: &[(&str, &str)]) -> Self {
        let bin = assert_cmd::cargo::cargo_bin("merge-ready");

        let mut cmd = std::process::Command::new(&bin);
        cmd.args(["daemon", "start"])
            .env("PATH", env.path_env())
            .env("HOME", env.home())
            .env("TMPDIR", env.home())
            .env("MERGE_READY_BASE_DIR", env.home())
            .env("XDG_CONFIG_HOME", env.home().join(".config"))
            .current_dir(env.repo.path())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        for (k, v) in extra_envs {
            cmd.env(k, v);
        }
        let mut child = cmd.spawn().expect("daemon spawn failed");

        // CLI 親プロセスの exit を待つ。socket ファイル存在ポーリングだと
        // (1) pid 書き込み前に socket が観測される race と
        // (2) 同一バージョンの旧 daemon stop ハンドラが新 daemon の socket を
        //     消してしまう race の双方を引き当てる。CLI の "ready" ハンドシェイク
        //     完了後の exit を待つことで両方を回避する。
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(START_WAIT_MS);
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    if status.success() {
                        return DaemonHandle::new(child, env.home_tmp.path().to_path_buf());
                    }
                    let stderr = read_pipe(child.stderr.as_mut());
                    let stdout = read_pipe(child.stdout.as_mut());
                    panic!(
                        "daemon CLI exited non-zero: status={:?}, stdout={stdout:?}, stderr={stderr:?}",
                        status.code()
                    );
                }
                Ok(None) => {
                    if std::time::Instant::now() >= deadline {
                        let _ = child.kill();
                        let stderr = read_pipe(child.stderr.as_mut());
                        let stdout = read_pipe(child.stdout.as_mut());
                        let _ = child.wait();
                        panic!(
                            "daemon did not start within {START_WAIT_MS}ms; stdout={stdout:?}, stderr={stderr:?}"
                        );
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(e) => panic!("daemon try_wait failed: {e}"),
            }
        }
    }

    /// キャッシュに有効な値が入るまで最大 `max_ms` ミリ秒ポーリングする。
    pub fn wait_for_cache(env: &TestEnv, max_ms: u64) {
        let bin = assert_cmd::cargo::cargo_bin("merge-ready-prompt");
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(max_ms);
        loop {
            let out = run_prompt_with_timeout(
                std::process::Command::new(&bin)
                    .env("PATH", env.path_env())
                    .env("HOME", env.home())
                    .env("TMPDIR", env.home())
                    .env("MERGE_READY_BASE_DIR", env.home())
                    .current_dir(env.repo.path()),
            );
            let stdout = String::from_utf8_lossy(&out.stdout);
            if stdout != "? loading" {
                return;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "cache not populated within {max_ms}ms"
            );
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }

    pub fn stop_for_env(env: &TestEnv) {
        Self::stop_base_dir(env.home());
    }

    fn stop_base_dir(base_dir: &Path) {
        let pid = read_pid(base_dir);
        let bin = assert_cmd::cargo::cargo_bin("merge-ready");
        let mut stop = std::process::Command::new(&bin);
        stop.args(["daemon", "stop"])
            .env("MERGE_READY_BASE_DIR", base_dir)
            .env("HOME", base_dir)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        let _ = run_command_to_exit(&mut stop, STOP_WAIT_MS);

        if let Some(pid) = pid
            && !wait_until_pid_gone(pid, STOP_WAIT_MS)
        {
            let _ = std::process::Command::new("kill")
                .args(["-TERM", &pid.to_string()])
                .status();
            let _ = wait_until_pid_gone(pid, STOP_WAIT_MS);
        }
        let _ = wait_until_socket_removed(base_dir, STOP_WAIT_MS);
    }
}

fn run_command_to_exit(cmd: &mut std::process::Command, max_ms: u64) -> bool {
    let Ok(mut child) = cmd.spawn() else {
        return false;
    };
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(max_ms);
    loop {
        if child.try_wait().is_ok_and(|status| status.is_some()) {
            return true;
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

impl Drop for DaemonHandle {
    fn drop(&mut self) {
        Self::stop_base_dir(&self.base_dir);
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

fn pid_path(base_dir: &Path) -> std::path::PathBuf {
    base_dir.join(format!("daemon-{}.pid", env!("CARGO_PKG_VERSION")))
}

fn socket_path(base_dir: &Path) -> std::path::PathBuf {
    base_dir.join(format!("daemon-{}.sock", env!("CARGO_PKG_VERSION")))
}

fn read_pid(base_dir: &Path) -> Option<u32> {
    fs::read_to_string(pid_path(base_dir))
        .ok()
        .and_then(|s| s.trim().parse().ok())
}

fn wait_until_pid_gone(pid: u32, max_ms: u64) -> bool {
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(max_ms);
    loop {
        if !is_pid_alive(pid) {
            return true;
        }
        if std::time::Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

fn is_pid_alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

fn read_pipe<R: Read>(pipe: Option<&mut R>) -> String {
    let Some(pipe) = pipe else {
        return String::new();
    };
    let mut buf = String::new();
    let _ = pipe.read_to_string(&mut buf);
    buf
}

fn wait_until_socket_removed(base_dir: &Path, max_ms: u64) -> bool {
    let socket = socket_path(base_dir);
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(max_ms);
    loop {
        if !socket.exists() {
            return true;
        }
        if std::time::Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}
