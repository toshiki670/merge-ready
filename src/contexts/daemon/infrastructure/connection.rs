use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::runtime::Handle;
use tokio::sync::mpsc::UnboundedSender;

use super::daemon_server::RefreshFn;
use super::daemon_state_actor::DaemonStateHandle;
use super::paths::Paths;
use super::request_handler::ActionResult;
use super::restart;
use crate::shared::protocol::Request;

pub(super) async fn handle(
    mut stream: UnixStream,
    state: &DaemonStateHandle,
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

    let Some(ActionResult {
        response,
        refresh_repo_id,
        refresh_cwd,
        stop,
    }) = state.process(request).await
    else {
        return;
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
