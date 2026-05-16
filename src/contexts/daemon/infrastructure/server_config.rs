use crate::contexts::daemon::domain::refresh_policy::RefreshPolicy;

// ── Hot モード ────────────────────────────────────────────────────────────────
/// 「最近 Query あり」と見なす経過秒数
const DEFAULT_HOT_RECENT_QUERY_SECS: u64 = 30;
/// Hot + 最近 Query あり の場合のリフレッシュ間隔
const DEFAULT_HOT_WITH_QUERY_SECS: u64 = 2;
/// Hot のみ（Query なし）の場合のリフレッシュ間隔
const DEFAULT_HOT_WITHOUT_QUERY_SECS: u64 = 10;

// ── Warm モード ───────────────────────────────────────────────────────────────
const DEFAULT_WARM_REFRESH_SECS: u64 = 180;
/// Warm から Cold へ移行するまでの Query 無し経過秒数
const DEFAULT_WARM_TO_COLD_SECS: u64 = 30 * 60;

// ── Cold モード ───────────────────────────────────────────────────────────────
/// Cold 初期（累計リフレッシュ `COLD_EARLY_LIMIT` 回まで）の間隔
const DEFAULT_COLD_EARLY_SECS: u64 = 30 * 60;
/// Cold 後期（`COLD_EARLY_LIMIT` 回超）の間隔
const DEFAULT_COLD_LATE_SECS: u64 = 60 * 60;
/// Cold 初期から後期へ切り替わる累計リフレッシュ回数
const DEFAULT_COLD_EARLY_LIMIT: u32 = 10;

// ── エントリ寿命 ──────────────────────────────────────────────────────────────
/// 最終 Query から この秒数が経過したエントリを削除する（2 日）
const DEFAULT_ENTRY_MAX_AGE_SECS: u64 = 2 * 24 * 60 * 60;

const DEFAULT_STALE_TTL_SECS: u64 = 5;
const DEFAULT_REFRESH_LOCK_TIMEOUT_SECS: u64 = 120;
const DEFAULT_SCHEDULER_TICK_SECS: u64 = 2;

// ── rate_limit 連動スケジューリング ────────────────────────────────────────
/// `MERGE_READY_RATE_LIMIT_AWARE` のデフォルト値。`0` / `false` で OFF。
const DEFAULT_RATE_LIMIT_AWARE: bool = true;

#[derive(Debug, Clone, Copy)]
pub(super) struct DaemonServerConfig {
    pub(super) stale_ttl_secs: u64,
    pub(super) refresh_lock_timeout_secs: u64,
    pub(super) entry_max_age_secs: u64,
    pub(super) scheduler_tick_secs: u64,
    pub(super) policy: RefreshPolicy,
    /// `gh api rate_limit` を観測して動的スケーリングと枯渇時 backoff を有効化する。
    #[allow(dead_code)] // 後続コミットで daemon_server が参照するまでの間
    pub(super) rate_limit_aware: bool,
}

impl DaemonServerConfig {
    pub(super) fn from_env() -> Self {
        Self {
            stale_ttl_secs: env_u64("MERGE_READY_STALE_TTL", DEFAULT_STALE_TTL_SECS),
            refresh_lock_timeout_secs: env_u64(
                "MERGE_READY_REFRESH_LOCK_TIMEOUT_SECS",
                DEFAULT_REFRESH_LOCK_TIMEOUT_SECS,
            ),
            entry_max_age_secs: env_u64(
                "MERGE_READY_ENTRY_MAX_AGE_SECS",
                DEFAULT_ENTRY_MAX_AGE_SECS,
            ),
            scheduler_tick_secs: env_u64(
                "MERGE_READY_SCHEDULER_TICK_SECS",
                DEFAULT_SCHEDULER_TICK_SECS,
            ),
            policy: RefreshPolicy {
                hot_recent_query_secs: env_u64(
                    "MERGE_READY_HOT_RECENT_QUERY_SECS",
                    DEFAULT_HOT_RECENT_QUERY_SECS,
                ),
                hot_with_query_secs: env_u64(
                    "MERGE_READY_HOT_WITH_QUERY_SECS",
                    DEFAULT_HOT_WITH_QUERY_SECS,
                ),
                hot_without_query_secs: env_u64(
                    "MERGE_READY_HOT_WITHOUT_QUERY_SECS",
                    DEFAULT_HOT_WITHOUT_QUERY_SECS,
                ),
                warm_refresh_secs: env_u64(
                    "MERGE_READY_WARM_REFRESH_SECS",
                    DEFAULT_WARM_REFRESH_SECS,
                ),
                warm_to_cold_secs: env_u64(
                    "MERGE_READY_WARM_TO_COLD_SECS",
                    DEFAULT_WARM_TO_COLD_SECS,
                ),
                cold_early_secs: env_u64("MERGE_READY_COLD_EARLY_SECS", DEFAULT_COLD_EARLY_SECS),
                cold_late_secs: env_u64("MERGE_READY_COLD_LATE_SECS", DEFAULT_COLD_LATE_SECS),
                cold_early_limit: env_u32("MERGE_READY_COLD_EARLY_LIMIT", DEFAULT_COLD_EARLY_LIMIT),
            },
            rate_limit_aware: env_bool("MERGE_READY_RATE_LIMIT_AWARE", DEFAULT_RATE_LIMIT_AWARE),
        }
    }
}

fn env_u64(key: &str, default: u64) -> u64 {
    parse_u64(std::env::var(key).ok().as_deref(), default)
}

fn env_u32(key: &str, default: u32) -> u32 {
    parse_u32(std::env::var(key).ok().as_deref(), default)
}

fn env_bool(key: &str, default: bool) -> bool {
    parse_bool(std::env::var(key).ok().as_deref(), default)
}

fn parse_u64(value: Option<&str>, default: u64) -> u64 {
    value.and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn parse_u32(value: Option<&str>, default: u32) -> u32 {
    value.and_then(|v| v.parse().ok()).unwrap_or(default)
}

/// `"0"`、`"false"`（大文字小文字無視）を false に解釈する。それ以外は default。
fn parse_bool(value: Option<&str>, default: bool) -> bool {
    match value {
        None => default,
        Some(v) => match v.trim().to_ascii_lowercase().as_str() {
            "0" | "false" | "no" | "off" => false,
            "1" | "true" | "yes" | "on" => true,
            _ => default,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_bool_unset_returns_default() {
        assert!(parse_bool(None, true));
        assert!(!parse_bool(None, false));
    }

    #[test]
    fn parse_bool_recognises_false_tokens() {
        for v in ["0", "false", "FALSE", "False", "no", "off"] {
            assert!(!parse_bool(Some(v), true), "expected false for {v:?}");
        }
    }

    #[test]
    fn parse_bool_recognises_true_tokens() {
        for v in ["1", "true", "TRUE", "yes", "on"] {
            assert!(parse_bool(Some(v), false), "expected true for {v:?}");
        }
    }

    #[test]
    fn parse_bool_unknown_returns_default() {
        assert!(parse_bool(Some("maybe"), true));
        assert!(!parse_bool(Some("xyz"), false));
    }
}
