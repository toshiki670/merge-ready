//! daemon 専用の `gh` プロセス起動ラッパ。
//!
//! evaluation コンテキストの `gh/command.rs` のロジックを意図的に複製している。
//! `run_gh` は `pub(super)` で context 境界を越えられず、project の cross-context
//! 参照ルール（DDD レイヤ依存規則）にも反するため、共有せず複製した。
//! `gh api rate_limit` 専用のため、評価用エラー分類（`AuthRequired` / `NoPr` /
//! `RateLimited` 等）は持たず最小構成にしている。

use std::io::{ErrorKind, Read};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub(super) enum GhCommandError {
    NotInstalled,
    Timeout,
    ApiError(String),
}

fn gh_timeout() -> Duration {
    let secs = std::env::var("MERGE_READY_GH_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(30);
    Duration::from_secs(secs)
}

pub(super) fn run_gh(args: &[&str]) -> Result<Vec<u8>, GhCommandError> {
    let mut cmd = Command::new("gh");
    cmd.args(args);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = match cmd.spawn() {
        Err(e) if e.kind() == ErrorKind::NotFound => return Err(GhCommandError::NotInstalled),
        Err(e) => return Err(GhCommandError::ApiError(e.to_string())),
        Ok(c) => c,
    };

    let mut stdout_pipe = child.stdout.take().expect("piped");
    let mut stderr_pipe = child.stderr.take().expect("piped");

    let (tx_out, rx_out) = mpsc::channel::<Vec<u8>>();
    let (tx_err, rx_err) = mpsc::channel::<Vec<u8>>();
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout_pipe.read_to_end(&mut buf);
        let _ = tx_out.send(buf);
    });
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stderr_pipe.read_to_end(&mut buf);
        let _ = tx_err.send(buf);
    });

    let deadline = Instant::now() + gh_timeout();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = rx_out.recv().unwrap_or_default();
                let stderr = rx_err.recv().unwrap_or_default();
                if status.success() {
                    return Ok(stdout);
                }
                let stderr_str = String::from_utf8_lossy(&stderr).into_owned();
                return Err(GhCommandError::ApiError(stderr_str));
            }
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(GhCommandError::Timeout);
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => return Err(GhCommandError::ApiError(e.to_string())),
        }
    }
}
