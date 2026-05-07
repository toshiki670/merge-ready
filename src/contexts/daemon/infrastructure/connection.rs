use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::daemon_server::RefreshFn;
use super::protocol::Request;
use super::request_handler::{self, ActionResult};
use super::restart;
use super::server_state::DaemonState;

pub(super) fn handle(
    mut stream: UnixStream,
    state: &Arc<Mutex<DaemonState>>,
    on_refresh: &RefreshFn,
    exit_tx: &mpsc::Sender<()>,
    restart_started: &Arc<AtomicBool>,
) {
    let mut buf = String::new();
    {
        let mut reader = BufReader::new(&stream);
        if reader.read_line(&mut buf).is_err() || buf.is_empty() {
            return;
        }
    }

    let request: Request = match serde_json::from_str(buf.trim()) {
        Ok(r) => r,
        Err(_) => return,
    };

    let ActionResult {
        response,
        refresh_repo_id,
        refresh_cwd,
        stop,
        restart_after_response,
    } = {
        let mut s = state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let config = s.config;
        let started_at = s.started_at;
        request_handler::process(
            &request,
            &mut s.entries,
            &config.policy,
            started_at,
            config.stale_ttl_secs,
        )
    };

    if let Ok(json) = serde_json::to_string(&response) {
        let _ = stream.write_all(format!("{json}\n").as_bytes());
    }
    drop(stream);

    if let (Some(repo_id), Some(cwd)) = (refresh_repo_id, refresh_cwd) {
        super::daemon_server::spawn_refresh(&repo_id, &cwd, on_refresh);
    }

    if restart_after_response {
        restart::restart_once(restart_started, exit_tx);
        return;
    }

    if stop {
        restart::cleanup();
        std::thread::sleep(Duration::from_millis(50));
        let _ = exit_tx.send(());
    }
}
