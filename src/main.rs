mod application;
mod domain;
mod infra;
mod presentation;

use application::errors::{ErrorPresenter, ErrorToken};
use infra::cache::CacheStatus;

fn main() {
    // テスト/デバッグ用: MERGE_READY_NO_CACHE=1 でキャッシュを無効化して gh を直接呼ぶ
    if std::env::var("MERGE_READY_NO_CACHE").as_deref() == Ok("1") {
        run_direct();
        return;
    }

    // バックグラウンドリフレッシュモード: gh を呼んでキャッシュを更新するだけ（stdout に出力しない）
    if std::env::args().any(|a| a == "--refresh") {
        if let Some(id) = infra::repo_id::get() {
            let output = run_silent();
            infra::cache::write(&id, &output);
        }
        return;
    }

    // 通常モード: gh を呼ばずキャッシュから即座に返す（常に <40ms）
    let Some(id) = infra::repo_id::get() else {
        // git remote が取得できない場合はローディング表示
        print!("? loading");
        return;
    };

    match infra::cache::check(&id) {
        CacheStatus::Fresh(output) => {
            print!("{output}");
        }
        CacheStatus::Stale(output) => {
            print!("{output}");
            spawn_background_refresh();
        }
        CacheStatus::Miss => {
            print!("? loading");
            spawn_background_refresh();
        }
    }
}

/// テスト/デバッグ用: gh を直接呼んで結果を stdout に出力する（従来の挙動）
fn run_direct() {
    let tokens = application::run(
        &infra::gh::GhClient::new(),
        &infra::logger::Logger,
        &presentation::Presenter,
    );
    if !tokens.is_empty() {
        presentation::display(&tokens);
    }
}

/// バックグラウンドリフレッシュ用: gh を呼んで結果を文字列で返す（stdout には何も出力しない）
fn run_silent() -> String {
    struct SilentPresenter;

    impl ErrorPresenter for SilentPresenter {
        fn show_error(&self, _token: ErrorToken) {}
    }

    let tokens = application::run(
        &infra::gh::GhClient::new(),
        &infra::logger::Logger,
        &SilentPresenter,
    );
    presentation::render_to_string(&tokens)
}

fn spawn_background_refresh() {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let _ = std::process::Command::new(exe)
        .arg("--refresh")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}
