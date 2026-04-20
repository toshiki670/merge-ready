use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::process::Stdio;
use std::time::Duration;

use super::paths;
use super::protocol::{Request, Response};
use crate::contexts::status_cache::domain::cache::{CachePort, CacheState};

/// デーモンソケットへの接続タイムアウト（ms）
const READ_TIMEOUT_MS: u64 = 500;

pub struct DaemonClient;

impl CachePort for DaemonClient {
    fn query(&self, repo_id: &str) -> Result<CacheState, ()> {
        let cwd = std::env::current_dir()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        match Self::send(&Request::Query {
            repo_id: repo_id.to_owned(),
            cwd,
        }) {
            Ok(Response::Fresh { output }) => Ok(CacheState::Fresh(output)),
            Ok(Response::Stale { output }) => Ok(CacheState::Stale(output)),
            Ok(Response::Miss) => Ok(CacheState::Miss),
            Ok(_) | Err(()) => {
                Self::lazy_start();
                Err(())
            }
        }
    }

    fn update(&self, repo_id: &str, output: &str) {
        let _ = Self::send(&Request::Update {
            repo_id: repo_id.to_owned(),
            output: output.to_owned(),
        });
    }
}

impl DaemonClient {
    pub(super) fn stop() -> bool {
        Self::send(&Request::Stop).is_ok()
    }

    pub(super) fn status_raw() -> Option<(u32, usize, u64)> {
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
        let _ = std::process::Command::new(&exe)
            .args(["daemon", "start"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    }
}

/// `DaemonClient` を使ってキャッシュを問い合わせる。
/// `Fresh(s)` → `Some(s)`（空文字 = PRなし）、`Stale("")` / `Miss` / 接続失敗 → `None`（ロード中）。
pub fn query_via_daemon(repo_id: &str) -> Option<String> {
    cache_state_to_output(DaemonClient.query(repo_id))
}

fn cache_state_to_output(state: Result<CacheState, ()>) -> Option<String> {
    match state {
        Ok(CacheState::Fresh(s)) => Some(s),
        Ok(CacheState::Stale(s)) if !s.is_empty() => Some(s),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_with_output_returns_some() {
        let result = cache_state_to_output(Ok(CacheState::Fresh("✓ merge-ready".into())));
        assert_eq!(result, Some("✓ merge-ready".into()));
    }

    #[test]
    fn stale_with_output_returns_some() {
        let result = cache_state_to_output(Ok(CacheState::Stale("✓ merge-ready".into())));
        assert_eq!(result, Some("✓ merge-ready".into()));
    }

    #[test]
    fn miss_returns_none() {
        let result = cache_state_to_output(Ok(CacheState::Miss));
        assert_eq!(result, None);
    }

    #[test]
    fn error_returns_none() {
        let result = cache_state_to_output(Err(()));
        assert_eq!(result, None);
    }

    #[test]
    fn fresh_empty_returns_some_empty() {
        // PRなし = キャッシュ済みの空文字列 → Some("") であり None ではない
        let result = cache_state_to_output(Ok(CacheState::Fresh(String::new())));
        assert_eq!(result, Some(String::new()));
    }

    #[test]
    fn stale_empty_returns_none() {
        // Stale("") はリフレッシュ中の空プレースホルダーの可能性があるため None
        let result = cache_state_to_output(Ok(CacheState::Stale(String::new())));
        assert_eq!(result, None);
    }
}
