use std::fs;
use std::io::Write as _;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::application::cache::{CachePort, CacheState};

const DEFAULT_STALE_TTL_SECS: u64 = 5;
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
///
/// ロック取得直後に自プロセスの PID を書き込むため、ロックファイルが空になる瞬間は存在しない。
/// 空ファイル＝クラッシュ等で書き込まれなかった異常状態として「死んでいる」と判定する。
pub fn try_acquire_refresh_lock(repo_id: &str) -> bool {
    let Some(path) = lock_path(repo_id) else {
        return false;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    if let Ok(mut f) = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    {
        // 取得直後に自 PID を書き込む（空ファイル期間をなくす）
        let _ = f.write_all(std::process::id().to_string().as_bytes());
        return true;
    }

    // ロックファイルが既存: 生存確認して死んでいれば再取得
    if is_lock_alive(&path) {
        return false;
    }
    let _ = fs::remove_file(&path);
    if let Ok(mut f) = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    {
        let _ = f.write_all(std::process::id().to_string().as_bytes());
        true
    } else {
        false
    }
}

/// spawn 後に子プロセスの PID をロックファイルへ上書きする。
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
/// - PID なし（空ファイル）→ 異常状態として dead 扱い
fn is_lock_alive(path: &std::path::Path) -> bool {
    let content = fs::read_to_string(path).unwrap_or_default();
    let trimmed = content.trim();

    if trimmed.is_empty() {
        return false;
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
