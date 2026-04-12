mod args;

use std::process::Stdio;

use crate::application::prompt::{PromptEffect, RepoIdPort};

pub(super) use args::PromptArgs;

struct InfraRepoIdPort;

impl RepoIdPort for InfraRepoIdPort {
    fn get(&self) -> Option<String> {
        crate::infra::repo_id::get()
    }
}

pub(crate) fn run(args: &PromptArgs) {
    if args.refresh {
        match args.repo_id.as_deref() {
            Some(id) => {
                // 親プロセスから --repo-id で渡された場合（通常パス）: git 再取得なし
                run_refresh(id);
            }
            None => {
                // 手動実行など親なしの場合: git から取得（この場合ロック孤児は発生しない）
                if let Some(id) = crate::infra::repo_id::get() {
                    run_refresh(&id);
                }
            }
        }
    } else if args.no_cache {
        run_direct();
    } else {
        run_cached();
    }
}

/// キャッシュ方針に基づいて表示し、必要に応じてバックグラウンドリフレッシュを起動する。
fn run_cached() {
    let cache = crate::infra::cache::CacheStore;
    match crate::application::prompt::resolve_cached(&InfraRepoIdPort, &cache) {
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

/// gh を直接呼んでキャッシュを更新する（stdout に出力しない）。
///
/// `repo_id` は親プロセスから `--repo-id` 引数で受け取る。
/// git から再取得しないため、ブランチ切替・git 一時失敗時でもロック解放が保証される。
fn run_refresh(repo_id: &str) {
    let tokens = crate::application::prompt::fetch_output(
        &crate::infra::gh::GhClient::new(),
        &crate::infra::logger::Logger,
    );
    if let Some(tokens) = tokens {
        let output = crate::presentation::render_to_string(&tokens);
        crate::infra::cache::write(repo_id, &output);
    }
    crate::infra::refresh_lock::release(repo_id);
}

/// ロックを取得できた場合のみバックグラウンドリフレッシュを起動する（多重起動抑止）。
///
/// spawn 時に `--repo-id` を渡すことで子プロセスが同じ `repo_id` でロックを解放できる。
fn spawn_refresh_if_needed(repo_id: &str) {
    if !crate::infra::refresh_lock::try_acquire(repo_id) {
        return;
    }
    let Ok(exe) = std::env::current_exe() else {
        crate::infra::refresh_lock::release(repo_id);
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
            // 子 PID をロックファイルへ書き込む（kill -0 による生存確認に使用）
            crate::infra::refresh_lock::update_pid(repo_id, child.id());
        }
        Err(_) => {
            crate::infra::refresh_lock::release(repo_id);
        }
    }
}

/// gh を直接呼んで結果を stdout に出力する（キャッシュを使わない）。
fn run_direct() {
    let tokens = crate::application::run(
        &crate::infra::gh::GhClient::new(),
        &crate::infra::logger::Logger,
        &crate::presentation::Presenter,
    );
    if !tokens.is_empty() {
        crate::presentation::display(&tokens);
    }
}
