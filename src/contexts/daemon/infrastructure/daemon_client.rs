use std::path::PathBuf;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::time::timeout;

use crate::contexts::daemon::domain::cache::{CachePort, RepoId};
use crate::shared::protocol::{EntryDto, PrOutput, Request, Response};
use crate::shared::refresh_mode::RefreshMode;

const READ_TIMEOUT_MS: u64 = 500;

pub struct DaemonClient {
    socket_path: PathBuf,
}

impl DaemonClient {
    pub fn new(socket_path: PathBuf) -> Self {
        Self { socket_path }
    }
}

impl CachePort for DaemonClient {
    async fn update(
        &self,
        repo_id: &RepoId,
        output: &str,
        refresh_mode: RefreshMode,
        pr_outputs: Vec<PrOutput>,
    ) {
        let _ = self
            .send(&Request::Update {
                repo_id: repo_id.as_str().to_owned(),
                output: output.to_owned(),
                refresh_mode,
                pr_outputs,
            })
            .await;
    }
}

impl DaemonClient {
    pub(super) async fn stop(&self) -> bool {
        self.send(&Request::Stop).await.is_ok()
    }

    pub(super) async fn status_raw(&self) -> Option<(usize, u64, String)> {
        match self.send(&Request::Status).await {
            Ok(Response::Status {
                entries,
                uptime_secs,
                version,
            }) => Some((entries, uptime_secs, version)),
            _ => None,
        }
    }

    pub(crate) async fn entries_raw(&self) -> Option<Vec<EntryDto>> {
        match self.send(&Request::Entries).await {
            Ok(Response::Entries { entries }) => Some(entries),
            _ => None,
        }
    }

    async fn send(&self, request: &Request) -> Result<Response, ()> {
        let stream = UnixStream::connect(&self.socket_path)
            .await
            .map_err(|_| ())?;
        let (read_half, mut write_half) = stream.into_split();

        let json = serde_json::to_string(request).map_err(|_| ())?;
        write_half
            .write_all(format!("{json}\n").as_bytes())
            .await
            .map_err(|_| ())?;

        let mut reader = BufReader::new(read_half);
        let mut buf = String::new();
        // std 版の set_read_timeout 相当を tokio::time::timeout で再現。
        timeout(
            Duration::from_millis(READ_TIMEOUT_MS),
            reader.read_line(&mut buf),
        )
        .await
        .map_err(|_| ())?
        .map_err(|_| ())?;

        serde_json::from_str(buf.trim()).map_err(|_| ())
    }
}
