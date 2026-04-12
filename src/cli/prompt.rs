use std::sync::atomic::{AtomicBool, Ordering};

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
///
/// git リポジトリでない場合は何も出力しない（`run_direct` と同じ挙動）。
fn run_cached() {
    let Some(repo_id) = crate::infra::repo_id::get() else {
        return;
    };
    let cache = crate::infra::cache::CacheStore;
    match crate::application::cache::resolve(&repo_id, &cache) {
        DisplayAction::Display(s) => {
            print!("{s}");
        }
        DisplayAction::DisplayAndRefresh(s) => {
            print!("{s}");
            maybe_spawn_refresh(&repo_id);
        }
        DisplayAction::LoadingWithRefresh => {
            print!("? loading");
            maybe_spawn_refresh(&repo_id);
        }
    }
}

/// gh を直接呼んでキャッシュを更新する（stdout に出力しない）
///
/// エラー発生時は既存キャッシュを上書きしない。
fn run_refresh() {
    let Some(id) = crate::infra::repo_id::get() else {
        return;
    };
    if let Some(output) = run_silent() {
        crate::infra::cache::write(&id, &output);
    }
    crate::infra::cache::release_refresh_lock(&id);
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

/// gh を呼んで結果を文字列で返す。エラー発生時は `None` を返す（空文字と区別するため）。
fn run_silent() -> Option<String> {
    struct TrackingPresenter(AtomicBool);

    impl ErrorPresenter for TrackingPresenter {
        fn show_error(&self, _token: ErrorToken) {
            self.0.store(true, Ordering::Relaxed);
        }
    }

    let presenter = TrackingPresenter(AtomicBool::new(false));
    let tokens = crate::application::run(
        &crate::infra::gh::GhClient::new(),
        &crate::infra::logger::Logger,
        &presenter,
    );

    if presenter.0.load(Ordering::Relaxed) {
        None
    } else {
        Some(crate::presentation::render_to_string(&tokens))
    }
}

/// ロックを取得できた場合のみバックグラウンドリフレッシュを起動する（多重起動抑止）
fn maybe_spawn_refresh(repo_id: &str) {
    if !crate::infra::cache::try_acquire_refresh_lock(repo_id) {
        return;
    }
    let Ok(exe) = std::env::current_exe() else {
        crate::infra::cache::release_refresh_lock(repo_id);
        return;
    };
    if std::process::Command::new(exe)
        .args(["prompt", "--refresh"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .is_err()
    {
        crate::infra::cache::release_refresh_lock(repo_id);
    }
    // 正常起動時はロックを子プロセス（run_refresh）が解放する
}
