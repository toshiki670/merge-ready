use std::sync::atomic::{AtomicBool, Ordering};

use crate::application::cache::DisplayAction;
use crate::application::errors::{ErrorPresenter, ErrorToken};

/// キャッシュ方針に基づいて表示し、必要に応じてバックグラウンドリフレッシュを起動する。
///
/// git リポジトリでない場合は何も出力しない。
pub fn run_cached() {
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
            crate::infra::refresh_lock::maybe_spawn_refresh(&repo_id);
        }
        DisplayAction::LoadingWithRefresh => {
            print!("? loading");
            crate::infra::refresh_lock::maybe_spawn_refresh(&repo_id);
        }
    }
}

/// gh を直接呼んでキャッシュを更新する（stdout に出力しない）。
///
/// エラー発生時は既存キャッシュを上書きしない。
pub fn run_refresh() {
    let Some(id) = crate::infra::repo_id::get() else {
        return;
    };
    if let Some(output) = fetch_silent() {
        crate::infra::cache::write(&id, &output);
    }
    crate::infra::refresh_lock::release(&id);
}

/// gh を直接呼んで結果を stdout に出力する（キャッシュを使わない）。
pub fn run_direct() {
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
fn fetch_silent() -> Option<String> {
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
