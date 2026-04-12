use std::sync::atomic::{AtomicBool, Ordering};

use crate::application::cache::{CachePort, DisplayAction};
use crate::application::errors::{ErrorPresenter, ErrorToken, ErrorLogger};
use crate::application::OutputToken;
use crate::domain::{
    branch_sync::BranchSyncRepository, ci_checks::CiChecksRepository,
    merge_ready::MergeReadinessRepository, pr_state::PrStateRepository, review::ReviewRepository,
};

/// git リポジトリ ID を取得するポート
pub trait RepoIdPort {
    fn get(&self) -> Option<String>;
}

/// キャッシュ表示の結果として CLI 層が実行すべき意図を表す
pub enum PromptEffect {
    /// そのまま表示（バックグラウンドリフレッシュ不要）
    Show(String),
    /// 表示してからバックグラウンドリフレッシュを要求する
    ShowAndRefresh { output: String, repo_id: String },
    /// "? loading" を表示してバックグラウンドリフレッシュを要求する（キャッシュミス）
    ShowLoadingAndRefresh { repo_id: String },
    /// 何も表示しない（git リポジトリ外など）
    NoOutput,
}

/// キャッシュ方針に基づいて表示意図を返す（副作用なし）。
///
/// git リポジトリ外の場合は [`PromptEffect::NoOutput`] を返す。
pub fn resolve_cached(repo_id: &impl RepoIdPort, cache: &impl CachePort) -> PromptEffect {
    let Some(id) = repo_id.get() else {
        return PromptEffect::NoOutput;
    };
    match crate::application::cache::resolve(&id, cache) {
        DisplayAction::Display(s) => PromptEffect::Show(s),
        DisplayAction::DisplayAndRefresh(s) => PromptEffect::ShowAndRefresh {
            output: s,
            repo_id: id,
        },
        DisplayAction::LoadingWithRefresh => PromptEffect::ShowLoadingAndRefresh { repo_id: id },
    }
}

/// gh を呼んで出力トークンを返す。エラー発生時は `None` を返す（キャッシュ書き込み回避）。
///
/// バックグラウンドリフレッシュ用。エラーは stderr に出力せず内部追跡のみ行う。
pub fn fetch_output<C, L>(client: &C, logger: &L) -> Option<Vec<OutputToken>>
where
    C: PrStateRepository
        + BranchSyncRepository
        + CiChecksRepository
        + ReviewRepository
        + MergeReadinessRepository
        + Sync,
    L: ErrorLogger + Sync,
{
    struct TrackingPresenter(AtomicBool);

    impl ErrorPresenter for TrackingPresenter {
        fn show_error(&self, _token: ErrorToken) {
            self.0.store(true, Ordering::Relaxed);
        }
    }

    let presenter = TrackingPresenter(AtomicBool::new(false));
    let tokens = crate::application::run(client, logger, &presenter);

    if presenter.0.load(Ordering::Relaxed) {
        None
    } else {
        Some(tokens)
    }
}
