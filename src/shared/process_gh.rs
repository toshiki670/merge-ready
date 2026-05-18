//! `gh` CLI のサブプロセス起動を一本化したラッパ。
//!
//! daemon / evaluation 双方の context から `crate::shared::process_gh::run_gh`
//! で呼び出す。タイムアウトは `MERGE_READY_GH_TIMEOUT_SECS`（既定 30 秒）。
//!
//! エラー分類は呼び出し側の関心事として保持し、ここでは
//! `NotInstalled` / `Timeout` / `Failed { exit_code, stderr }` の 3 種類だけを返す。

use std::io::{ErrorKind, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub enum GhProcessError {
    /// `gh` バイナリが見つからない
    NotInstalled,
    /// `MERGE_READY_GH_TIMEOUT_SECS` を超えた
    Timeout,
    /// 非ゼロ終了。`exit_code` と `stderr` を呼び出し側の分類器に渡せる
    Failed { exit_code: i32, stderr: String },
}

fn gh_timeout() -> Duration {
    let secs = std::env::var("MERGE_READY_GH_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(30);
    Duration::from_secs(secs)
}

pub fn run_gh(args: &[&str], cwd: Option<&Path>) -> Result<Vec<u8>, GhProcessError> {
    let mut cmd = Command::new("gh");
    cmd.args(args);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }

    let mut child = match cmd.spawn() {
        Err(e) if e.kind() == ErrorKind::NotFound => return Err(GhProcessError::NotInstalled),
        Err(e) => {
            return Err(GhProcessError::Failed {
                exit_code: -1,
                stderr: e.to_string(),
            });
        }
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
                let exit_code = status.code().unwrap_or(1);
                let stderr_str = String::from_utf8_lossy(&stderr).into_owned();
                return Err(GhProcessError::Failed {
                    exit_code,
                    stderr: stderr_str,
                });
            }
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(GhProcessError::Timeout);
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => {
                return Err(GhProcessError::Failed {
                    exit_code: -1,
                    stderr: e.to_string(),
                });
            }
        }
    }
}
