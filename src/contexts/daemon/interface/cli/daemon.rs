use std::io::{BufRead, BufReader, Read};
use std::process::{ExitCode, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use clap::{Args, Subcommand};

use crate::contexts::daemon::application::lifecycle::{self, Port};

const DAEMON_INNER_ENV: &str = "MERGE_READY_DAEMON_INNER";
const START_TIMEOUT_SECS: u64 = 2;

#[derive(Subcommand, Clone, Copy)]
pub enum DaemonCommand {
    /// Start the background cache daemon
    Start,
    /// Stop the running daemon
    Stop,
    /// Show daemon status
    Status,
}

#[derive(Args, Clone, Copy)]
pub struct DaemonArgs {
    #[command(subcommand)]
    pub subcommand: DaemonCommand,
}

pub fn run(subcommand: DaemonCommand, port: &impl Port) -> ExitCode {
    match subcommand {
        DaemonCommand::Start => start(port),
        DaemonCommand::Stop => stop(port),
        DaemonCommand::Status => status(port),
    }
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
fn start(port: &impl Port) -> ExitCode {
    if std::env::var(DAEMON_INNER_ENV).is_ok() {
        return match lifecycle::start(port) {
            Ok(()) => ExitCode::SUCCESS,
            Err(_) => ExitCode::FAILURE,
        };
    }
    let Ok(exe) = std::env::current_exe() else {
        eprintln!("merge-ready: failed to locate executable");
        return ExitCode::FAILURE;
    };
    let mut child = match std::process::Command::new(exe)
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
        let _ = child.kill();
        eprintln!("merge-ready: failed to capture daemon stdout");
        return ExitCode::FAILURE;
    };
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let mut reader = BufReader::new(stdout);
        let _ = reader.read_line(&mut line);
        let _ = tx.send(line);
    });

    let deadline = Instant::now() + Duration::from_secs(START_TIMEOUT_SECS);
    loop {
        // 内側プロセスが早期終了した場合（already running / bind 失敗等）
        if let Ok(Some(status)) = child.try_wait() {
            // 捕捉した stderr を外側の stderr へ中継する
            if let Some(mut err) = child.stderr.take() {
                let mut buf = String::new();
                let _ = err.read_to_string(&mut buf);
                if !buf.is_empty() {
                    eprint!("{buf}");
                }
            }
            let code = if status.success() {
                1u8
            } else {
                u8::try_from(status.code().unwrap_or(1)).unwrap_or(1)
            };
            return ExitCode::from(code);
        }
        if matches!(rx.try_recv().ok().as_deref(), Some("ready\n")) {
            println!("daemon started");
            return ExitCode::SUCCESS;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            eprintln!("merge-ready: daemon did not start within {START_TIMEOUT_SECS}s");
            return ExitCode::FAILURE;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn stop(port: &impl Port) -> ExitCode {
    if lifecycle::stop(port) {
        println!("daemon stopped");
    } else {
        eprintln!("daemon is not running");
    }
    ExitCode::SUCCESS
}

fn status(port: &impl Port) -> ExitCode {
    match lifecycle::get_status(port) {
        Some(s) => {
            let pid = lifecycle::get_pid(port).map_or_else(|| "-".to_owned(), |p| p.to_string());
            println!(
                "running  pid={}  entries={}  uptime={}s  version={}",
                pid, s.entries, s.uptime_secs, s.version
            );
        }
        None => println!("not running"),
    }
    ExitCode::SUCCESS
}
