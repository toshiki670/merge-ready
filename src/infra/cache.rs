use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

const DEFAULT_STALE_TTL_SECS: u64 = 5;
const CACHE_DIR_NAME: &str = "merge-ready";

#[derive(Serialize, Deserialize)]
struct StateJson {
    fetched_at_secs: u64,
    output: String,
}

/// キャッシュの状態
pub enum CacheStatus {
    /// `stale_ttl` 以内: 即出力、バックグラウンドリフレッシュなし
    Fresh(String),
    /// `stale_ttl` 超過: 即出力 + バックグラウンドリフレッシュ
    Stale(String),
    /// キャッシュなし: `? loading` を表示してバックグラウンドリフレッシュ
    Miss,
}

/// `repo_id` に対応するキャッシュの状態を返す。
///
/// キャッシュファイルが存在しない、または読み込めない場合は [`CacheStatus::Miss`] を返す。
pub fn check(repo_id: &str) -> CacheStatus {
    let Some(state_path) = cache_path(repo_id) else {
        return CacheStatus::Miss;
    };

    let Ok(content) = fs::read_to_string(&state_path) else {
        return CacheStatus::Miss;
    };

    let Ok(state) = serde_json::from_str::<StateJson>(&content) else {
        return CacheStatus::Miss;
    };

    let now = now_secs();
    let age = now.saturating_sub(state.fetched_at_secs);

    if age <= stale_ttl_secs() {
        CacheStatus::Fresh(state.output)
    } else {
        CacheStatus::Stale(state.output)
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

fn cache_path(repo_id: &str) -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(
        std::path::Path::new(&home)
            .join(".cache")
            .join(CACHE_DIR_NAME)
            .join(repo_id)
            .join("state.json"),
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
