use std::process::Stdio;

use clap::Subcommand;

use crate::contexts::status_cache::application::lifecycle::{self, Port};

const DAEMON_INNER_ENV: &str = "MERGE_READY_DAEMON_INNER";

#[derive(Subcommand, Clone, Copy)]
pub enum DaemonCommand {
    /// Start the background cache daemon
    Start,
    /// Stop the running daemon
    Stop,
    /// Show daemon status
    Status,
}

pub fn run(subcommand: DaemonCommand, port: &impl Port) {
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
fn start(port: &impl Port) {
    if std::env::var(DAEMON_INNER_ENV).is_ok() {
        lifecycle::start(port);
        return;
    }
    let Ok(exe) = std::env::current_exe() else {
        eprintln!("merge-ready: failed to locate executable");
        std::process::exit(1);
    };
    let _ = std::process::Command::new(exe)
        .args(["daemon", "start"])
        .env(DAEMON_INNER_ENV, "1")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

fn stop(port: &impl Port) {
    if lifecycle::stop(port) {
        println!("daemon stopped");
    } else {
        eprintln!("daemon is not running");
    }
}

fn status(port: &impl Port) {
    match lifecycle::get_status(port) {
        Some(s) => {
            println!(
                "running  pid={}  entries={}  uptime={}s",
                s.pid, s.entries, s.uptime_secs
            );
        }
        None => println!("not running"),
    }
}
