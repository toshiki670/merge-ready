use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

use crate::contexts::daemon::domain::cache::{CachePort, RepoId};
use crate::shared::protocol::{EntryDto, PrOutput, Request, Response};
use crate::shared::refresh_mode::RefreshMode;

/// デーモンソケットへの接続タイムアウト（ms）
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
    fn update(
        &self,
        repo_id: &RepoId,
        output: &str,
        refresh_mode: RefreshMode,
        pr_outputs: Vec<PrOutput>,
    ) {
        let _ = self.send(&Request::Update {
            repo_id: repo_id.as_str().to_owned(),
            output: output.to_owned(),
            refresh_mode,
            pr_outputs,
        });
    }
}

impl DaemonClient {
    pub(super) fn stop(&self) -> bool {
        self.send(&Request::Stop).is_ok()
    }

    pub(super) fn status_raw(&self) -> Option<(usize, u64, String)> {
        match self.send(&Request::Status) {
            Ok(Response::Status {
                entries,
                uptime_secs,
                version,
            }) => Some((entries, uptime_secs, version)),
            _ => None,
        }
    }

    pub(crate) fn entries_raw(&self) -> Option<Vec<EntryDto>> {
        match self.send(&Request::Entries) {
            Ok(Response::Entries { entries }) => Some(entries),
            _ => None,
        }
    }

    fn send(&self, request: &Request) -> Result<Response, ()> {
        let stream = UnixStream::connect(&self.socket_path).map_err(|_| ())?;
        stream
            .set_read_timeout(Some(Duration::from_millis(READ_TIMEOUT_MS)))
            .map_err(|_| ())?;
        let mut stream = stream;

        let json = serde_json::to_string(request).map_err(|_| ())?;
        stream
            .write_all(format!("{json}\n").as_bytes())
            .map_err(|_| ())?;

        let mut buf = String::new();
        BufReader::new(&stream)
            .read_line(&mut buf)
            .map_err(|_| ())?;

        serde_json::from_str(buf.trim()).map_err(|_| ())
    }
}
