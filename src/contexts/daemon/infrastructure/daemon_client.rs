use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

use super::paths;
use super::protocol::{RefreshModeDto, Request, Response};
use crate::contexts::daemon::domain::cache::{CachePort, RefreshMode, RepoId};

/// デーモンソケットへの接続タイムアウト（ms）
const READ_TIMEOUT_MS: u64 = 500;

pub struct DaemonClient;

impl From<RefreshMode> for RefreshModeDto {
    fn from(m: RefreshMode) -> Self {
        match m {
            RefreshMode::Hot => RefreshModeDto::Hot,
            RefreshMode::Warm => RefreshModeDto::Warm,
            RefreshMode::Terminal => RefreshModeDto::Terminal,
        }
    }
}

impl CachePort for DaemonClient {
    fn update(&self, repo_id: &RepoId, output: &str, refresh_mode: RefreshMode) {
        let _ = Self::send(&Request::Update {
            repo_id: repo_id.as_str().to_owned(),
            output: output.to_owned(),
            refresh_mode: RefreshModeDto::from(refresh_mode),
        });
    }
}

impl DaemonClient {
    pub(super) fn stop() -> bool {
        Self::send(&Request::Stop).is_ok()
    }

    pub(super) fn status_raw() -> Option<(usize, u64, String)> {
        match Self::send(&Request::Status) {
            Ok(Response::Status {
                entries,
                uptime_secs,
                version,
            }) => Some((entries, uptime_secs, version)),
            _ => None,
        }
    }

    fn send(request: &Request) -> Result<Response, ()> {
        let stream = UnixStream::connect(paths::socket_path()).map_err(|_| ())?;
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
