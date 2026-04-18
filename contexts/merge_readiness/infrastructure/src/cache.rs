use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use merge_readiness_application::cache::{CachePort, CacheState};

use crate::tmp_cache_dir;

const DEFAULT_STALE_TTL_SECS: u64 = 5;

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
    let state_path = cache_path(repo_id);

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
    let state_path = cache_path(repo_id);

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

    // PID ベースの tmp 名でプロセス間衝突を防ぎ、rename でアトミック置換（部分書き込み防止）
    let temp_path = state_path.with_extension(format!("tmp.{}", std::process::id()));
    if fs::write(&temp_path, &content).is_err() {
        return;
    }
    if fs::rename(&temp_path, &state_path).is_err() {
        let _ = fs::remove_file(&temp_path); // rename 失敗時に tmp 残留防止
    }
}

fn cache_path(repo_id: &str) -> std::path::PathBuf {
    tmp_cache_dir::cache_dir().join(format!("{repo_id}.json"))
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
