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

pub(super) fn stale_ttl_secs() -> u64 {
    env_u64("MERGE_READY_STALE_TTL", DEFAULT_STALE_TTL_SECS)
}

pub(super) fn refresh_lock_timeout_secs() -> u64 {
    env_u64(
        "MERGE_READY_REFRESH_LOCK_TIMEOUT_SECS",
        DEFAULT_REFRESH_LOCK_TIMEOUT_SECS,
    )
}

pub(super) fn hot_recent_query_secs() -> u64 {
    env_u64(
        "MERGE_READY_HOT_RECENT_QUERY_SECS",
        DEFAULT_HOT_RECENT_QUERY_SECS,
    )
}

pub(super) fn hot_with_query_secs() -> u64 {
    env_u64(
        "MERGE_READY_HOT_WITH_QUERY_SECS",
        DEFAULT_HOT_WITH_QUERY_SECS,
    )
}

pub(super) fn hot_without_query_secs() -> u64 {
    env_u64(
        "MERGE_READY_HOT_WITHOUT_QUERY_SECS",
        DEFAULT_HOT_WITHOUT_QUERY_SECS,
    )
}

pub(super) fn warm_refresh_secs() -> u64 {
    env_u64("MERGE_READY_WARM_REFRESH_SECS", DEFAULT_WARM_REFRESH_SECS)
}

pub(super) fn warm_to_cold_secs() -> u64 {
    env_u64("MERGE_READY_WARM_TO_COLD_SECS", DEFAULT_WARM_TO_COLD_SECS)
}

pub(super) fn cold_early_secs() -> u64 {
    env_u64("MERGE_READY_COLD_EARLY_SECS", DEFAULT_COLD_EARLY_SECS)
}

pub(super) fn cold_late_secs() -> u64 {
    env_u64("MERGE_READY_COLD_LATE_SECS", DEFAULT_COLD_LATE_SECS)
}

pub(super) fn cold_early_limit() -> u32 {
    std::env::var("MERGE_READY_COLD_EARLY_LIMIT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_COLD_EARLY_LIMIT)
}

pub(super) fn entry_max_age_secs() -> u64 {
    env_u64("MERGE_READY_ENTRY_MAX_AGE_SECS", DEFAULT_ENTRY_MAX_AGE_SECS)
}

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}
