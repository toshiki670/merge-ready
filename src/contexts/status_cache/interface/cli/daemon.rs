use crate::contexts::status_cache::infrastructure::{daemon_client::DaemonClient, daemon_server, pid};

/// デーモンを起動する。すでに起動中の場合はエラーを表示して終了する。
pub fn start() {
    if let Some(p) = pid::read() {
        if pid::is_alive(p) {
            eprintln!("merge-ready daemon is already running (pid {p})");
            std::process::exit(1);
        }
        // 古い PID ファイルを除去してから起動
        pid::remove();
    }
    daemon_server::run();
}

/// デーモンを停止する。
pub fn stop() {
    if DaemonClient::stop() {
        println!("daemon stopped");
        return;
    }

    let Some(p) = pid::read() else {
        eprintln!("daemon is not running");
        return;
    };

    if !pid::is_alive(p) {
        eprintln!("daemon is not running (stale pid file removed)");
        pid::remove();
        return;
    }

    // ソケット経由が失敗した場合は SIGTERM でフォールバック
    let ok = std::process::Command::new("kill")
        .args(["-TERM", &p.to_string()])
        .status()
        .is_ok_and(|s| s.success());

    if ok {
        println!("daemon stopped (pid {p})");
    } else {
        eprintln!("failed to stop daemon (pid {p})");
        std::process::exit(1);
    }
}

/// デーモンのステータスを表示する。
pub fn status() {
    match DaemonClient::status_info() {
        Some((pid, entries, uptime_secs)) => {
            println!("running  pid={pid}  entries={entries}  uptime={uptime_secs}s");
        }
        None => {
            println!("not running");
        }
    }
}
