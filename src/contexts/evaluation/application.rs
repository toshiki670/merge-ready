mod branch_sync;
mod ci_checks;
pub mod errors;
pub mod port;
mod pr_state;
pub mod prompt;
mod review;
mod unblocked;

use crate::contexts::evaluation::domain::pr_state::blocked::BlockedState;
use crate::contexts::evaluation::domain::pr_state::blocked::branch_sync::BranchSyncState;
use crate::contexts::evaluation::domain::pr_state::blocked::ci::CiState;
use crate::contexts::evaluation::domain::pr_state::blocked::review::ReviewState;
use crate::contexts::evaluation::domain::pr_state::unblocked::UnblockedState;
use crate::contexts::evaluation::domain::pr_state::{EvaluationInput, PrState, evaluate, is_open};
use errors::{ErrorLogger, ErrorPresenter};
use port::PromptStatusPort;

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

fn map_blocked_to_tokens(blocked: BlockedState) -> Vec<OutputToken> {
    let mut tokens = Vec::new();
    if let Some(s) = blocked.branch_sync {
        tokens.push(match s {
            BranchSyncState::Conflict => OutputToken::Conflict,
            BranchSyncState::UpdateBranch => OutputToken::UpdateBranch,
            BranchSyncState::SyncUnknown => OutputToken::SyncUnknown,
        });
    }
    if let Some(c) = blocked.ci {
        tokens.push(match c {
            CiState::Fail => OutputToken::CiFail,
            CiState::ActionRequired => OutputToken::CiAction,
        });
    }
    if let Some(r) = blocked.review {
        tokens.push(match r {
            ReviewState::ChangesRequested => OutputToken::ReviewRequested,
        });
    }
    tokens
}

fn map_pr_state_to_tokens(state: PrState) -> Vec<OutputToken> {
    match state {
        PrState::Blocked(blocked) => map_blocked_to_tokens(blocked),
        PrState::Unblocked(UnblockedState::MergeReady) => vec![OutputToken::MergeReady],
        // Draft (#154)、NoPr (#156) は後続 Issue で実装
        PrState::Unblocked(UnblockedState::Draft)
        | PrState::NoPr
        | PrState::NotApplicable
        | PrState::Unknown => vec![],
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
    C: PromptStatusPort + Sync,
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
    // review と unblocked はキャッシュ済みの pr_view データを使用するため追加呼び出しなし
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
    let Some(readiness) = unblocked::fetch(client, err_logger, err_presenter) else {
        return vec![];
    };

    let pr_state = evaluate(&EvaluationInput {
        branch_sync: &sync_status,
        ci_checks: &buckets,
        review: &review_status,
        readiness: &readiness,
    });

    map_pr_state_to_tokens(pr_state)
}
