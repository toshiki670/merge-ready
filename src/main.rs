mod application;
mod domain;
mod infra;
mod presentation;

use application::cache::DisplayAction;

fn main() {
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
