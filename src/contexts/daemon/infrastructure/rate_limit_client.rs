//! `gh api rate_limit` の取得・60 秒キャッシュ・JSON パースを担当する。
//!
//! 取得自体はクォータ非消費なので、定期取得しても観測対象のリソースを
//! 消費しない。`fetch_or_cached` は cache hit のときに `gh` を起動しない。

use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant, UNIX_EPOCH};

use serde::Deserialize;

use super::gh_command::{GhCommandError, run_gh};
use crate::contexts::daemon::domain::rate_limit_snapshot::RateLimitSnapshot;

#[allow(dead_code)] // 後続コミットで daemon_server が起動するまでの間
pub(super) struct RateLimitClient {
    cache: Mutex<Option<RateLimitSnapshot>>,
    ttl: Duration,
}

#[derive(Deserialize)]
struct RateLimitResponse {
    resources: Resources,
}

#[derive(Deserialize)]
struct Resources {
    core: ResourceInfo,
    graphql: ResourceInfo,
}

#[derive(Deserialize)]
struct ResourceInfo {
    limit: u32,
    remaining: u32,
    reset: u64,
}

#[allow(dead_code)]
impl RateLimitClient {
    pub(super) fn new(ttl: Duration) -> Self {
        Self {
            cache: Mutex::new(None),
            ttl,
        }
    }

    /// キャッシュが新鮮なら再利用し、そうでなければ `gh api rate_limit` を呼ぶ。
    /// 取得失敗時は `None` を返す（呼び出し側でフォールバック挙動）。
    pub(super) fn fetch_or_cached(&self) -> Option<RateLimitSnapshot> {
        if let Some(snap) = self.cached_if_fresh() {
            return Some(snap);
        }
        self.force_refresh()
    }

    /// 強制的に再取得する（403 受信時の即時参照用）。
    pub(super) fn force_refresh(&self) -> Option<RateLimitSnapshot> {
        let snap = raw_fetch()?;
        let mut guard = self.cache.lock().unwrap_or_else(PoisonError::into_inner);
        *guard = Some(snap);
        Some(snap)
    }

    fn cached_if_fresh(&self) -> Option<RateLimitSnapshot> {
        let guard = self.cache.lock().unwrap_or_else(PoisonError::into_inner);
        let snap = (*guard)?;
        if snap.fetched_at.elapsed() < self.ttl {
            Some(snap)
        } else {
            None
        }
    }
}

fn raw_fetch() -> Option<RateLimitSnapshot> {
    match run_gh(&["api", "rate_limit"]) {
        Ok(bytes) => {
            let parsed = parse_rate_limit_json(&bytes, Instant::now());
            if parsed.is_none() {
                log::warn!("rate_limit fetch: failed to parse gh response");
            }
            parsed
        }
        Err(GhCommandError::ApiError(msg)) => {
            log::warn!("rate_limit fetch failed: {msg}");
            None
        }
        Err(GhCommandError::NotInstalled) => {
            log::warn!("rate_limit fetch failed: gh not installed");
            None
        }
        Err(GhCommandError::Timeout) => {
            log::warn!("rate_limit fetch failed: timeout");
            None
        }
    }
}

fn parse_rate_limit_json(bytes: &[u8], fetched_at: Instant) -> Option<RateLimitSnapshot> {
    let response: RateLimitResponse = serde_json::from_slice(bytes).ok()?;
    Some(RateLimitSnapshot {
        core_remaining: response.resources.core.remaining,
        core_limit: response.resources.core.limit,
        graphql_remaining: response.resources.graphql.remaining,
        graphql_limit: response.resources.graphql.limit,
        reset_at: UNIX_EPOCH + Duration::from_secs(response.resources.core.reset),
        fetched_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    const SAMPLE_JSON: &[u8] = br#"{
        "resources": {
            "core": {"limit": 5000, "used": 50, "remaining": 4950, "reset": 1778924570},
            "search": {"limit": 30, "used": 0, "remaining": 30, "reset": 1778921589},
            "graphql": {"limit": 5000, "used": 400, "remaining": 4600, "reset": 1778921661}
        },
        "rate": {"limit": 5000, "used": 50, "remaining": 4950, "reset": 1778924570}
    }"#;

    #[test]
    fn parse_rate_limit_extracts_core_and_graphql() {
        let snap = parse_rate_limit_json(SAMPLE_JSON, Instant::now()).expect("parses");
        assert_eq!(snap.core_remaining, 4950);
        assert_eq!(snap.core_limit, 5000);
        assert_eq!(snap.graphql_remaining, 4600);
        assert_eq!(snap.graphql_limit, 5000);
        // reset_at は core.reset の unix epoch
        let expected = UNIX_EPOCH + Duration::from_secs(1_778_924_570);
        assert_eq!(snap.reset_at, expected);
    }

    #[test]
    fn parse_rate_limit_rejects_invalid_json() {
        assert!(parse_rate_limit_json(b"not json", Instant::now()).is_none());
    }

    #[test]
    fn parse_rate_limit_rejects_missing_fields() {
        let json = br#"{"resources": {"core": {"limit": 5000}}}"#;
        assert!(parse_rate_limit_json(json, Instant::now()).is_none());
    }

    #[test]
    fn cached_if_fresh_returns_none_initially() {
        let client = RateLimitClient::new(Duration::from_mins(1));
        assert!(client.cached_if_fresh().is_none());
    }

    #[test]
    fn cached_if_fresh_returns_snapshot_within_ttl() {
        let client = RateLimitClient::new(Duration::from_mins(1));
        let snap = RateLimitSnapshot {
            core_remaining: 100,
            core_limit: 5000,
            graphql_remaining: 100,
            graphql_limit: 5000,
            reset_at: SystemTime::now() + Duration::from_hours(1),
            fetched_at: Instant::now(),
        };
        *client.cache.lock().unwrap() = Some(snap);
        assert!(client.cached_if_fresh().is_some());
    }

    #[test]
    fn cached_if_fresh_returns_none_when_stale() {
        let client = RateLimitClient::new(Duration::from_secs(1));
        let snap = RateLimitSnapshot {
            core_remaining: 100,
            core_limit: 5000,
            graphql_remaining: 100,
            graphql_limit: 5000,
            reset_at: SystemTime::now() + Duration::from_hours(1),
            // 2 秒前に取得 → TTL 1 秒では stale
            fetched_at: Instant::now()
                .checked_sub(Duration::from_secs(2))
                .expect("past"),
        };
        *client.cache.lock().unwrap() = Some(snap);
        assert!(client.cached_if_fresh().is_none());
    }
}
