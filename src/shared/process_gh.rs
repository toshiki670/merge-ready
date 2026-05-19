//! `gh` CLI のサブプロセス起動を一本化したラッパ。
//!
//! daemon / evaluation 双方の context から `crate::shared::process_gh::run_gh`
//! で呼び出す。タイムアウトは `MERGE_READY_GH_TIMEOUT_SECS`（既定 30 秒）。
//!
//! エラー分類は呼び出し側の関心事として保持し、ここでは
//! `NotInstalled` / `Timeout` / `Failed { exit_code, stderr }` の 3 種類だけを返す。

use std::io::ErrorKind;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;
use tokio::time::timeout;

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

pub async fn run_gh(args: &[&str], cwd: Option<&Path>) -> Result<Vec<u8>, GhProcessError> {
    let mut cmd = Command::new("gh");
    cmd.args(args);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    // kill_on_drop=true で、タイムアウトで future を drop した瞬間に
    // 子プロセスへ SIGKILL を送る。明示的な kill+wait のボイラープレートを削減。
    cmd.kill_on_drop(true);

    let child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) if e.kind() == ErrorKind::NotFound => return Err(GhProcessError::NotInstalled),
        Err(e) => panic!("spawn gh: {e}"),
    };

    let output = match timeout(gh_timeout(), child.wait_with_output()).await {
        Ok(out) => out.expect("wait_with_output on gh child"),
        Err(_) => return Err(GhProcessError::Timeout),
    };

    if output.status.success() {
        Ok(output.stdout)
    } else {
        let exit_code = output.status.code().unwrap_or(1);
        let stderr_str = String::from_utf8_lossy(&output.stderr).into_owned();
        Err(GhProcessError::Failed {
            exit_code,
            stderr: stderr_str,
        })
    }
}
