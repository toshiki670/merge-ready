use std::process::Stdio;

use crate::contexts::merge_readiness::application::prompt::PromptEffect;
use crate::contexts::merge_readiness::infrastructure::{cache::CacheStore, refresh_lock};
use crate::contexts::status_cache::application::cache::{self, CacheQueryResult};
use crate::contexts::status_cache::infrastructure::daemon_client::DaemonClient;

use crate::InfraRepoIdPort;

/// キャッシュ方針に基づいて表示し、必要に応じてバックグラウンドリフレッシュを起動する。
///
/// デーモンが起動中の場合は Unix ソケット経由でキャッシュを取得する（sub-ms パス）。
/// デーモンが未起動の場合はファイルキャッシュへフォールバックしてバックグラウンドリフレッシュを起動する。
pub fn run() {
    let Some(repo_id) = crate::contexts::merge_readiness::infrastructure::repo_id::get() else {
        return;
    };

    // デーモン経由の高速パス
    let client = DaemonClient;
    match cache::query(&client, &repo_id) {
        CacheQueryResult::Fresh(s) | CacheQueryResult::Stale(s) => {
            // デーモンがリフレッシュを内部管理するため追加処理不要
            print!("{s}");
            return;
        }
        CacheQueryResult::Miss => {
            // デーモンがリフレッシュを内部で予約済み
            print!("? loading");
            return;
        }
        CacheQueryResult::Unavailable => {
            // フォールバック: ファイルキャッシュ（デーモン未起動時）
        }
    }

    run_with_file_cache();
}

/// ファイルキャッシュを使う従来パス（デーモン未起動時のフォールバック）。
fn run_with_file_cache() {
    let cache = CacheStore;
    match crate::contexts::merge_readiness::application::prompt::resolve_cached(
        &InfraRepoIdPort,
        &cache,
    ) {
        PromptEffect::NoOutput => {}
        PromptEffect::Show(s) => print!("{s}"),
        PromptEffect::ShowAndRefresh { output, repo_id } => {
            print!("{output}");
            spawn_refresh_if_needed(&repo_id);
        }
        PromptEffect::ShowLoadingAndRefresh { repo_id } => {
            print!("? loading");
            spawn_refresh_if_needed(&repo_id);
        }
    }
}

/// ロックを取得できた場合のみバックグラウンドリフレッシュを起動する（多重起動抑止）。
///
/// spawn 時に `--repo-id` を渡すことで子プロセスが同じ `repo_id` でロックを解放できる。
fn spawn_refresh_if_needed(repo_id: &str) {
    if !refresh_lock::try_acquire(repo_id) {
        return;
    }
    let Ok(exe) = std::env::current_exe() else {
        refresh_lock::release(repo_id);
        return;
    };
    match std::process::Command::new(exe)
        .args(["prompt", "--refresh", "--repo-id", repo_id])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => {
            refresh_lock::update_pid(repo_id, child.id());
        }
        Err(_) => {
            refresh_lock::release(repo_id);
        }
    }
}
