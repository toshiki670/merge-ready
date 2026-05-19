use std::process::{ExitCode, Stdio};
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::Command;

use crate::contexts::daemon::domain::daemon::DaemonLifecyclePort;

const DAEMON_INNER_ENV: &str = "MERGE_READY_DAEMON_INNER";
const START_TIMEOUT_SECS: u64 = 2;

enum StartOutcome {
    Ready,
    EarlyExit(std::io::Result<std::process::ExitStatus>),
    Timeout,
}

// Why double-spawn instead of alternatives:
//
// - `daemonize` crate: RUSTSEC-2025-0069 (unmaintained) で cargo-deny に弾かれる
// - `libc`/`nix` の fork 直呼び: `unsafe_code = "forbid"` により使用不可
// - systemd/launchd: OS 依存。unit/plist ファイルの生成・登録が必要で複雑
//
// double-spawn は safe Rust のみで実現できる唯一の手段。
// 欠点は setsid() を呼べないため SIGHUP を受ける可能性があること。
// ただしプロンプト統合の用途では端末クローズ時にデーモンが終了しても
// 次回 prompt 呼び出し時に lazy_start() が再起動するため実害はない。
pub(crate) async fn start(port: &impl DaemonLifecyclePort) -> ExitCode {
    if std::env::var(DAEMON_INNER_ENV).is_ok() {
        return match port.start().await {
            Ok(()) => ExitCode::SUCCESS,
            Err(_) => ExitCode::FAILURE,
        };
    }
    let Ok(exe) = std::env::current_exe() else {
        eprintln!("merge-ready: failed to locate executable");
        return ExitCode::FAILURE;
    };
    let mut child = match Command::new(exe)
        .args(["daemon", "start"])
        .env(DAEMON_INNER_ENV, "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        // inherit ではなく piped にする。inherit だと内側プロセスが外側の stderr fd
        // のコピーを保持したまま走り続けるため、assert_cmd が EOF 待ちでハングする。
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("merge-ready: failed to spawn daemon: {e}");
            return ExitCode::FAILURE;
        }
    };

    let Some(stdout) = child.stdout.take() else {
        let _ = child.kill().await;
        eprintln!("merge-ready: failed to capture daemon stdout");
        return ExitCode::FAILURE;
    };
    let stderr = child.stderr.take();
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();

    let outcome = tokio::select! {
        res = reader.read_line(&mut line) => {
            if res.is_ok() && line == "ready\n" {
                StartOutcome::Ready
            } else {
                // EOF / 「ready\n」以外: 内側プロセスは既に終了したか終了直前。
                // 完全な exit status を取得するため明示的に wait する。
                StartOutcome::EarlyExit(child.wait().await)
            }
        }
        status = child.wait() => StartOutcome::EarlyExit(status),
        () = tokio::time::sleep(Duration::from_secs(START_TIMEOUT_SECS)) => StartOutcome::Timeout,
    };

    match outcome {
        StartOutcome::Ready => {
            println!("daemon started");
            ExitCode::SUCCESS
        }
        StartOutcome::EarlyExit(status) => {
            if let Some(mut err) = stderr {
                let mut buf = String::new();
                let _ = err.read_to_string(&mut buf).await;
                if !buf.is_empty() {
                    eprint!("{buf}");
                }
            }
            match status {
                Ok(s) if s.success() => ExitCode::from(1u8),
                Ok(s) => {
                    let code = u8::try_from(s.code().unwrap_or(1)).unwrap_or(1);
                    ExitCode::from(code)
                }
                Err(_) => ExitCode::FAILURE,
            }
        }
        StartOutcome::Timeout => {
            let _ = child.kill().await;
            eprintln!("merge-ready: daemon did not start within {START_TIMEOUT_SECS}s");
            ExitCode::FAILURE
        }
    }
}

pub(crate) fn stop(port: &impl DaemonLifecyclePort) -> ExitCode {
    if port.stop() {
        println!("daemon stopped");
    } else {
        eprintln!("daemon is not running");
    }
    ExitCode::SUCCESS
}

pub(crate) fn status(port: &impl DaemonLifecyclePort) -> ExitCode {
    match port.get_status() {
        Some(s) => {
            let pid = port
                .get_pid()
                .map_or_else(|| "-".to_owned(), |p| p.to_string());
            println!(
                "running  pid={}  entries={}  uptime={}s  version={}",
                pid, s.entries, s.uptime_secs, s.version
            );
        }
        None => println!("not running"),
    }
    ExitCode::SUCCESS
}
