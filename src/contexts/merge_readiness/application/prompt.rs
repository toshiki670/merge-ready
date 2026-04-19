use std::sync::atomic::{AtomicBool, Ordering};

use super::OutputToken;
use super::cache::{CachePort, DisplayAction};
use super::errors::{ErrorLogger, ErrorPresenter, ErrorToken};
use crate::contexts::merge_readiness::domain::{
    branch_sync::BranchSyncRepository, ci_checks::CiChecksRepository,
    merge_ready::MergeReadinessRepository, pr_state::PrStateRepository, review::ReviewRepository,
};

/// git リポジトリ ID を取得するポート
pub trait RepoIdPort {
    fn get(&self) -> Option<String>;
}

/// プロンプト表示の実行モード
pub enum ExecutionMode {
    /// キャッシュを使わず gh を直接呼ぶ
    Direct,
    /// daemon 経由でキャッシュを参照する
    Cached,
}

/// キャッシュ表示の結果として CLI 層が実行すべき意図を表す
pub enum PromptEffect {
    /// そのまま表示（daemon がリフレッシュを内部管理）
    Show(String),
    /// stale な値を表示（daemon がリフレッシュを内部予約済み）
    ShowAndRefresh(String),
    /// "? loading" を表示（daemon がリフレッシュを内部予約済み）
    ShowLoadingAndRefresh,
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
    match super::cache::resolve(&id, cache) {
        DisplayAction::Display(s) => PromptEffect::Show(s),
        DisplayAction::DisplayAndRefresh(s) => PromptEffect::ShowAndRefresh(s),
        DisplayAction::LoadingWithRefresh => PromptEffect::ShowLoadingAndRefresh,
    }
}

/// gh を呼んで出力トークンを返す。エラー発生時は `None` を返す（daemon 書き込み回避）。
///
/// `daemon refresh` 処理用。エラーは stderr に出力せず内部追跡のみ行う。
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
            self.0
                .update(Ordering::Relaxed, Ordering::Relaxed, |_| true);
        }
    }

    let presenter = TrackingPresenter(AtomicBool::new(false));
    let tokens = super::run(client, logger, &presenter);

    if presenter.0.load(Ordering::Relaxed) {
        None
    } else {
        Some(tokens)
    }
}
