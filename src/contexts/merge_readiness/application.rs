mod branch_sync;
mod ci_checks;
pub mod errors;
mod merge_ready;
mod pr_state;
pub mod prompt;
mod review;

// interface 層が application のみに依存できるよう domain トレイトを再エクスポート
pub use super::domain::branch_sync::BranchSyncRepository;
pub use super::domain::ci_checks::CiChecksRepository;
pub use super::domain::merge_ready::MergeReadinessRepository;
pub use super::domain::pr_state::PrStateRepository;
pub use super::domain::review::ReviewRepository;

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
    if !pr_state::is_open(&lifecycle) {
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

    let mut tokens: Vec<OutputToken> = Vec::new();
    for token in [
        branch_sync::check(&sync_status),
        ci_checks::check(&buckets),
        review::check(&review_status),
    ]
    .into_iter()
    .flatten()
    {
        let _ = tokens.push_mut(token);
    }

    // ブロッカーがなければマージ可否を判定
    if tokens.is_empty()
        && let Some(token) = merge_ready::check(&readiness)
    {
        let _ = tokens.push_mut(token);
    }

    tokens
}
