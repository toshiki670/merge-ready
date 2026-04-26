pub mod config_service;
pub mod errors;
pub mod prompt;

use crate::contexts::evaluation::domain::pr_state::PrState;
use crate::contexts::evaluation::domain::pr_state::blocked::BlockedState;
use crate::contexts::evaluation::domain::pr_state::blocked::branch_sync::BranchSyncState;
use crate::contexts::evaluation::domain::pr_state::blocked::ci::CiState;
use crate::contexts::evaluation::domain::pr_state::blocked::review::ReviewState;
use crate::contexts::evaluation::domain::pr_state::unblocked::UnblockedState;

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

pub(crate) fn map_pr_state_to_tokens(state: PrState) -> Vec<OutputToken> {
    match state {
        PrState::Blocked(blocked) => map_blocked_to_tokens(blocked),
        PrState::Unblocked(UnblockedState::MergeReady) => vec![OutputToken::MergeReady],
        // Draft (#154)、NoPr (#156) は後続 Issue で実装
        // NotApplicable / Unknown は何も表示しない
        PrState::Unblocked(UnblockedState::Draft)
        | PrState::NoPr
        | PrState::NotApplicable(_)
        | PrState::Unknown => vec![],
    }
}
