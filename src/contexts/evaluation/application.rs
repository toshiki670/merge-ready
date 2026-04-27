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
    CiPending,
    ReviewRequested,
    ReviewRequired,
    MergeReady,
    NoPullRequest,
    Draft,
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
            CiState::Pending => OutputToken::CiPending,
        });
    }
    if let Some(r) = blocked.review {
        tokens.push(match r {
            ReviewState::ChangesRequested => OutputToken::ReviewRequested,
            ReviewState::ReviewRequired => OutputToken::ReviewRequired,
        });
    }
    tokens
}

pub(crate) fn map_pr_state_to_tokens(state: PrState) -> Vec<OutputToken> {
    match state {
        PrState::Blocked(blocked) => map_blocked_to_tokens(blocked),
        PrState::Unblocked(UnblockedState::MergeReady) => vec![OutputToken::MergeReady],
        PrState::Unblocked(UnblockedState::Draft) => vec![OutputToken::Draft],
        PrState::NoPr => vec![OutputToken::NoPullRequest],
        // NotApplicable / Unknown は何も表示しない
        PrState::NotApplicable(_) | PrState::Unknown => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_pr_maps_to_no_pull_request_token() {
        let tokens = map_pr_state_to_tokens(PrState::NoPr);
        assert_eq!(tokens.len(), 1);
        assert!(matches!(tokens[0], OutputToken::NoPullRequest));
    }

    #[test]
    fn draft_pr_maps_to_draft_token() {
        let tokens = map_pr_state_to_tokens(PrState::Unblocked(UnblockedState::Draft));
        assert_eq!(tokens.len(), 1);
        assert!(matches!(tokens[0], OutputToken::Draft));
    }

    #[test]
    fn review_required_maps_to_review_required_token() {
        use crate::contexts::evaluation::domain::pr_state::blocked::BlockedState;
        let blocked = BlockedState {
            branch_sync: None,
            ci: None,
            review: Some(ReviewState::ReviewRequired),
        };
        let tokens = map_pr_state_to_tokens(PrState::Blocked(blocked));
        assert_eq!(tokens.len(), 1);
        assert!(matches!(tokens[0], OutputToken::ReviewRequired));
    }

    #[test]
    fn ci_pending_maps_to_ci_pending_token() {
        use crate::contexts::evaluation::domain::pr_state::blocked::BlockedState;
        let blocked = BlockedState {
            branch_sync: None,
            ci: Some(CiState::Pending),
            review: None,
        };
        let tokens = map_pr_state_to_tokens(PrState::Blocked(blocked));
        assert_eq!(tokens.len(), 1);
        assert!(matches!(tokens[0], OutputToken::CiPending));
    }
}
