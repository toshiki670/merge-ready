use crate::application::prompt::{PromptEffect, RepoIdPort};
use crate::cli::args::PromptArgs;

struct InfraRepoIdPort;

impl RepoIdPort for InfraRepoIdPort {
    fn get(&self) -> Option<String> {
        crate::infra::repo_id::get()
    }
}

pub(crate) fn run(args: &PromptArgs) {
    if args.refresh {
        run_refresh();
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
            crate::infra::refresh_lock::maybe_spawn_refresh(&repo_id);
        }
        PromptEffect::ShowLoadingAndRefresh { repo_id } => {
            print!("? loading");
            crate::infra::refresh_lock::maybe_spawn_refresh(&repo_id);
        }
    }
}

/// gh を直接呼んでキャッシュを更新する（stdout に出力しない）。
///
/// エラー発生時は既存キャッシュを上書きしない。
fn run_refresh() {
    let Some(repo_id) = crate::infra::repo_id::get() else {
        return;
    };
    let tokens = crate::application::prompt::fetch_output(
        &crate::infra::gh::GhClient::new(),
        &crate::infra::logger::Logger,
    );
    if let Some(tokens) = tokens {
        let output = crate::presentation::render_to_string(&tokens);
        crate::infra::cache::write(&repo_id, &output);
    }
    crate::infra::refresh_lock::release(&repo_id);
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
