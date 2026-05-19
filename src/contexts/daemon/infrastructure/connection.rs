use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::runtime::Handle;
use tokio::sync::mpsc::UnboundedSender;

use super::daemon_server::RefreshFn;
use super::paths::Paths;
use super::request_handler::{self, ActionResult};
use super::restart;
use super::server_state::DaemonState;
use crate::shared::protocol::Request;

pub(super) async fn handle(
    mut stream: UnixStream,
    state: &Arc<Mutex<DaemonState>>,
    on_refresh: RefreshFn,
    exit_tx: &UnboundedSender<()>,
    paths: &Paths,
    handle: &Handle,
) {
    let mut buf = String::new();
    {
        let mut reader = BufReader::new(&mut stream);
        if reader.read_line(&mut buf).await.is_err() || buf.is_empty() {
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
    } = {
        let mut s = state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let config = s.config;
        let started_at = s.started_at;
        request_handler::process(
            &request,
            &mut s.cache_store,
            &config.policy,
            started_at,
            config.stale_ttl_secs,
        )
    };

    if let Ok(json) = serde_json::to_string(&response) {
        let _ = stream.write_all(format!("{json}\n").as_bytes()).await;
    }
    drop(stream);

    if let (Some(repo_id), Some(cwd)) = (refresh_repo_id, refresh_cwd) {
        super::daemon_server::spawn_refresh(&repo_id, &cwd, on_refresh, handle);
    }

    if stop {
        restart::cleanup(paths);
        tokio::time::sleep(Duration::from_millis(50)).await;
        let _ = exit_tx.send(());
    }
}
