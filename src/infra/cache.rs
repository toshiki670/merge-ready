use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::application::cache::{CachePort, CacheState};

const DEFAULT_STALE_TTL_SECS: u64 = 5;
/// PID 未書き込み状態のロックファイルに対する猶予期間（親プロセスがクラッシュした場合の安全弁）
const EMPTY_LOCK_TTL_SECS: u64 = 5;
const CACHE_DIR_NAME: &str = "merge-ready";

#[derive(Serialize, Deserialize)]
struct StateJson {
    fetched_at_secs: u64,
    output: String,
}

/// キャッシュストア（ファイルシステムバックエンド）
pub struct CacheStore;

impl CachePort for CacheStore {
    fn check(&self, repo_id: &str) -> CacheState {
        match check_raw(repo_id) {
            RawCacheStatus::Fresh(s) => CacheState::Fresh(s),
            RawCacheStatus::Stale(s) => CacheState::Stale(s),
            RawCacheStatus::Miss => CacheState::Miss,
        }
    }
}

/// キャッシュの内部状態（infra 層内部でのみ使用）
enum RawCacheStatus {
    Fresh(String),
    Stale(String),
    Miss,
}

/// `repo_id` に対応するキャッシュの状態を返す。
///
/// キャッシュファイルが存在しない、または読み込めない場合は [`RawCacheStatus::Miss`] を返す。
fn check_raw(repo_id: &str) -> RawCacheStatus {
    let Some(state_path) = cache_path(repo_id) else {
        return RawCacheStatus::Miss;
    };

    let Ok(content) = fs::read_to_string(&state_path) else {
        return RawCacheStatus::Miss;
    };

    let Ok(state) = serde_json::from_str::<StateJson>(&content) else {
        return RawCacheStatus::Miss;
    };

    let now = now_secs();
    let age = now.saturating_sub(state.fetched_at_secs);

    if age <= stale_ttl_secs() {
        RawCacheStatus::Fresh(state.output)
    } else {
        RawCacheStatus::Stale(state.output)
    }
}

/// キャッシュに出力文字列を書き込む。
///
/// ディレクトリが存在しない場合は自動的に作成する。
/// 書き込み失敗は静かに握り潰す。
pub fn write(repo_id: &str, output: &str) {
    let Some(state_path) = cache_path(repo_id) else {
        return;
    };

    if let Some(parent) = state_path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let state = StateJson {
        fetched_at_secs: now_secs(),
        output: output.to_owned(),
    };

    let Ok(content) = serde_json::to_string(&state) else {
        return;
    };

    let _ = fs::write(state_path, content);
}

/// リフレッシュロックを取得する。成功時は `true`、既に起動中なら `false` を返す。
///
/// ロックファイルが存在する場合は PID で生存確認（`kill -0`）を行い、
/// プロセスが死んでいれば stale とみなして除去し再取得する。
/// これにより gh が 30 秒超かかる場合でも重複起動を防げる。
pub fn try_acquire_refresh_lock(repo_id: &str) -> bool {
    let Some(path) = lock_path(repo_id) else {
        return false;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    if path.exists() && is_lock_alive(&path) {
        return false;
    }
    let _ = fs::remove_file(&path);

    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .is_ok()
}

/// spawn 後に子プロセスの PID をロックファイルへ書き込む。
pub fn update_lock_pid(repo_id: &str, pid: u32) {
    if let Some(path) = lock_path(repo_id) {
        let _ = fs::write(path, pid.to_string());
    }
}

/// リフレッシュロックを解放する。
pub fn release_refresh_lock(repo_id: &str) {
    if let Some(path) = lock_path(repo_id) {
        let _ = fs::remove_file(path);
    }
}

/// ロックファイルが示すプロセスが生存しているかを確認する。
///
/// - PID あり → `kill -0 <pid>` でプロセス生存確認
/// - PID なし（直前に取得済みで未書き込み）→ mtime が `EMPTY_LOCK_TTL_SECS` 以内なら生存扱い
fn is_lock_alive(path: &std::path::Path) -> bool {
    let content = fs::read_to_string(path).unwrap_or_default();
    let trimmed = content.trim();

    if trimmed.is_empty() {
        // 親が lock を取得した直後で PID 未書き込み状態。短い猶予期間で生存扱いとする
        return fs::metadata(path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| SystemTime::now().duration_since(t).ok())
            .is_some_and(|age| age.as_secs() < EMPTY_LOCK_TTL_SECS);
    }

    trimmed.parse::<u32>().is_ok_and(|pid| {
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    })
}

fn lock_path(repo_id: &str) -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(
        std::path::Path::new(&home)
            .join(".cache")
            .join(CACHE_DIR_NAME)
            .join(format!("{repo_id}.lock")),
    )
}

fn cache_path(repo_id: &str) -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(
        std::path::Path::new(&home)
            .join(".cache")
            .join(CACHE_DIR_NAME)
            .join(format!("{repo_id}.json")),
    )
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn stale_ttl_secs() -> u64 {
    std::env::var("MERGE_READY_STALE_TTL")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_STALE_TTL_SECS)
}
