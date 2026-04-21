use std::sync::atomic::{AtomicBool, Ordering};

use super::OutputToken;
use super::errors::{ErrorLogger, ErrorPresenter, ErrorToken};
use crate::contexts::merge_readiness::domain::{
    branch_sync::BranchSyncRepository, ci_checks::CiChecksRepository,
    merge_ready::MergeReadinessRepository, pr_state::PrStateRepository, review::ReviewRepository,
};

/// プロンプト表示の実行モード
pub enum ExecutionMode {
    /// キャッシュを使わず gh を直接呼ぶ
    Direct,
    /// daemon 経由でキャッシュを参照する
    Cached,
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
