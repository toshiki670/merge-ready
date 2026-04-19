use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::process::Stdio;
use std::time::Duration;

use super::paths;
use super::protocol::{Request, Response};
use crate::contexts::status_cache::domain::CacheQueryResult;

/// デーモンソケットへの接続タイムアウト（ms）
const READ_TIMEOUT_MS: u64 = 500;

pub struct DaemonClient;

impl DaemonClient {
    /// キャッシュを問い合わせる。
    ///
    /// デーモンが応答しない場合はバックグラウンドで起動を試み、[`CacheQueryResult::Unavailable`] を返す。
    pub fn query(repo_id: &str) -> CacheQueryResult {
        match Self::send(&Request::Query {
            repo_id: repo_id.to_owned(),
        }) {
            Ok(Response::Fresh { output }) => CacheQueryResult::Fresh(output),
            Ok(Response::Stale { output }) => CacheQueryResult::Stale(output),
            Ok(Response::Miss) => CacheQueryResult::Miss,
            Ok(_) | Err(()) => {
                Self::lazy_start();
                CacheQueryResult::Unavailable
            }
        }
    }

    /// キャッシュを更新する（fire-and-forget）。
    ///
    /// デーモンが起動していない場合は静かに無視する。
    pub fn update(repo_id: &str, output: &str) {
        let _ = Self::send(&Request::Update {
            repo_id: repo_id.to_owned(),
            output: output.to_owned(),
        });
    }

    /// デーモンに停止を要求する。応答を受け取れた場合 `true` を返す。
    pub fn stop() -> bool {
        Self::send(&Request::Stop).is_ok()
    }

    /// デーモンのステータスを取得する。起動していない場合は `None` を返す。
    #[must_use]
    pub fn status_info() -> Option<(u32, usize, u64)> {
        match Self::send(&Request::Status) {
            Ok(Response::Status {
                pid,
                entries,
                uptime_secs,
            }) => Some((pid, entries, uptime_secs)),
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

    fn lazy_start() {
        let Ok(exe) = std::env::current_exe() else {
            return;
        };
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            let _ = std::process::Command::new(&exe)
                .args(["daemon", "start"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .process_group(0)
                .spawn();
        }
        #[cfg(not(unix))]
        {
            let _ = std::process::Command::new(&exe)
                .args(["daemon", "start"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn();
        }
    }
}
