mod branch_sync;
mod ci_checks;
pub mod errors;
mod merge_ready;
mod pr_state;
pub mod prompt;
mod review;

pub trait BranchSyncRepository: super::domain::branch_sync::BranchSyncRepository {}
impl<T> BranchSyncRepository for T where T: super::domain::branch_sync::BranchSyncRepository {}

pub trait CiChecksRepository: super::domain::ci_checks::CiChecksRepository {}
impl<T> CiChecksRepository for T where T: super::domain::ci_checks::CiChecksRepository {}

pub trait MergeReadinessRepository: super::domain::merge_ready::MergeReadinessRepository {}
impl<T> MergeReadinessRepository for T where T: super::domain::merge_ready::MergeReadinessRepository {}

pub trait PrStateRepository: super::domain::pr_state::PrStateRepository {}
impl<T> PrStateRepository for T where T: super::domain::pr_state::PrStateRepository {}

pub trait ReviewRepository: super::domain::review::ReviewRepository {}
impl<T> ReviewRepository for T where T: super::domain::review::ReviewRepository {}

use crate::contexts::prompt::domain::policy::{PromptDecisionPolicy, PromptEvaluation};
use crate::contexts::prompt::domain::pr_state::is_open;
use crate::contexts::prompt::domain::signal::PromptSignal;
use errors::{ErrorLogger, ErrorPresenter};

/// アプリケーション層が返す出力トークンの意味オブジェクト
///
/// 文字列表現への変換は presentation 層が担う。
pub enum OutputToken {
    Conflict,
    UpdateBranch,
    SyncUnknown,
    CiFail,
    CiAction,
    ReviewRequested,
    MergeReady,
}

fn map_signal_to_output_token(signal: PromptSignal) -> OutputToken {
    match signal {
        PromptSignal::Conflict => OutputToken::Conflict,
        PromptSignal::UpdateBranch => OutputToken::UpdateBranch,
        PromptSignal::SyncUnknown => OutputToken::SyncUnknown,
        PromptSignal::CiFail => OutputToken::CiFail,
        PromptSignal::CiAction => OutputToken::CiAction,
        PromptSignal::ReviewRequested => OutputToken::ReviewRequested,
        PromptSignal::MergeReady => OutputToken::MergeReady,
    }
}

/// PR マージ可否チェックのユースケース
///
/// 表示すべきトークンを返す。呼び出し元が表示処理を担う。
/// PR が対象外（クローズ等）または取得失敗の場合は空 `Vec` を返す。
///
/// `branch_sync` と `ci_checks` のフェッチは独立した gh 呼び出しを必要とするため、
/// `std::thread::scope` を使って並列実行する。
///
/// # Panics
/// スレッドがパニックした場合（内部エラー）。
pub fn run<C, L, P>(client: &C, err_logger: &L, err_presenter: &P) -> Vec<OutputToken>
where
    C: PrStateRepository
        + BranchSyncRepository
        + CiChecksRepository
        + ReviewRepository
        + MergeReadinessRepository
        + Sync,
    L: ErrorLogger + Sync,
    P: ErrorPresenter + Sync,
{
    let Some(lifecycle) = pr_state::fetch(client, err_logger, err_presenter) else {
        return vec![];
    };
    if !is_open(&lifecycle) {
        return vec![];
    }

    // branch_sync と ci_checks は独立した gh 呼び出しを必要とするため並列フェッチ
    // review と merge_ready はキャッシュ済みの pr_view データを使用するため追加呼び出しなし
    let (sync_result, ci_result) = std::thread::scope(|s| {
        let sync_handle = s.spawn(|| branch_sync::fetch(client));
        let ci_handle = s.spawn(|| ci_checks::fetch(client));
        (
            sync_handle.join().expect("branch_sync thread panicked"),
            ci_handle.join().expect("ci_checks thread panicked"),
        )
    });

    // 両方失敗した場合でも err_presenter への通知は 1 回だけ（重複表示を防ぐ）
    let (sync_status, buckets) = match (sync_result, ci_result) {
        (Ok(s), Ok(b)) => (s, b),
        (Err(e), _) | (_, Err(e)) => {
            errors::handle(e, err_logger, err_presenter);
            return vec![];
        }
    };

    let Some(review_status) = review::fetch(client, err_logger, err_presenter) else {
        return vec![];
    };
    let Some(readiness) = merge_ready::fetch(client, err_logger, err_presenter) else {
        return vec![];
    };

    PromptDecisionPolicy::evaluate(&PromptEvaluation {
        branch_sync: &sync_status,
        ci_checks: &buckets,
        review: &review_status,
        readiness: &readiness,
    })
    .into_iter()
    .map(map_signal_to_output_token)
    .collect()
}
