use std::process::Stdio;

use merge_readiness_application::prompt::PromptEffect;
use merge_readiness_infrastructure::{cache::CacheStore, refresh_lock};

use crate::InfraRepoIdPort;

/// キャッシュ方針に基づいて表示し、必要に応じてバックグラウンドリフレッシュを起動する。
pub fn run() {
    let cache = CacheStore;
    match merge_readiness_application::prompt::resolve_cached(&InfraRepoIdPort, &cache) {
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
