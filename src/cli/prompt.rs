use crate::application::cache::DisplayAction;
use crate::application::errors::{ErrorPresenter, ErrorToken};
use crate::cli::args::PromptArgs;

pub(crate) fn run(args: &PromptArgs) {
    if args.refresh {
        run_refresh();
    } else if args.no_cache {
        run_direct();
    } else {
        run_cached();
    }
}

/// キャッシュ方針に基づいて表示し、必要に応じてバックグラウンドリフレッシュを起動する
fn run_cached() {
    let cache = crate::infra::cache::CacheStore;
    match crate::application::cache::resolve(crate::infra::repo_id::get().as_deref(), &cache) {
        DisplayAction::Display(s) => {
            print!("{s}");
        }
        DisplayAction::DisplayAndRefresh(s) => {
            print!("{s}");
            spawn_background_refresh();
        }
        DisplayAction::Loading => {
            print!("? loading");
            spawn_background_refresh();
        }
    }
}

/// gh を直接呼んでキャッシュを更新する（stdout に出力しない）
fn run_refresh() {
    let Some(id) = crate::infra::repo_id::get() else {
        return;
    };
    let output = run_silent();
    crate::infra::cache::write(&id, &output);
}

/// gh を直接呼んで結果を stdout に出力する（キャッシュを使わない）
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

/// gh を呼んで結果を文字列で返す（stdout には何も出力しない）
fn run_silent() -> String {
    struct SilentPresenter;

    impl ErrorPresenter for SilentPresenter {
        fn show_error(&self, _token: ErrorToken) {}
    }

    let tokens = crate::application::run(
        &crate::infra::gh::GhClient::new(),
        &crate::infra::logger::Logger,
        &SilentPresenter,
    );
    crate::presentation::render_to_string(&tokens)
}

fn spawn_background_refresh() {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let _ = std::process::Command::new(exe)
        .args(["prompt", "--refresh"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}
