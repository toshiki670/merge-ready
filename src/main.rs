mod application;
mod domain;
mod infra;
mod presentation;

use application::cache::DisplayAction;

fn main() {
    // テスト/デバッグ用: MERGE_READY_NO_CACHE=1 でキャッシュを無効化して gh を直接呼ぶ
    if std::env::var("MERGE_READY_NO_CACHE").as_deref() == Ok("1") {
        run_direct();
        return;
    }

    // 通常モード: キャッシュ方針はアプリケーション層が決定する
    let cache = infra::cache::CacheStore;
    match application::cache::resolve(infra::repo_id::get().as_deref(), &cache) {
        DisplayAction::Display(s) | DisplayAction::DisplayAndRefresh(s) => {
            print!("{s}");
        }
        DisplayAction::Loading => {
            print!("? loading");
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
