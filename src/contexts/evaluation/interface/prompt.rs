use crate::contexts::evaluation::application::config_service;
use crate::contexts::evaluation::application::errors::ErrorToken;
use crate::contexts::evaluation::application::port::ErrorLogger;
use crate::contexts::evaluation::application::prompt::fetch;
use crate::contexts::evaluation::domain::display_config::{
    DisplayConfig, DisplayConfigRepository, render_error_token, render_token,
};
use crate::contexts::evaluation::domain::pr_state::NotApplicableState;
use crate::contexts::evaluation::domain::pr_state::PrRepository;
use crate::contexts::evaluation::domain::pr_state::PrState;
use crate::contexts::evaluation::domain::pr_state::blocked::BlockedState;
use crate::contexts::evaluation::domain::pr_state::blocked::GenericBlockedState;
use crate::contexts::evaluation::domain::pr_state::blocked::branch_sync::BranchSyncState;
use crate::contexts::evaluation::domain::pr_state::blocked::ci::CiState;
use crate::contexts::evaluation::domain::pr_state::blocked::review::ReviewState;
use crate::contexts::evaluation::domain::pr_state::unblocked::UnblockedState;

pub fn render<R, C, L>(repo: &R, config_repo: &C, logger: &L) -> (String, bool)
where
    R: PrRepository,
    C: DisplayConfigRepository,
    L: ErrorLogger,
{
    let config = config_service::load(config_repo);
    match fetch(repo, logger) {
        Ok((state, is_terminal)) => (render_pr_state(state, &config), is_terminal),
        Err(token) => (render_error(&token, &config), false),
    }
}

fn render_pr_state(state: PrState, config: &DisplayConfig) -> String {
    let parts = match state {
        PrState::Blocked(blocked) => render_blocked(blocked, config),
        PrState::Unblocked(UnblockedState::MergeReady) => vec![render_token(&config.merge_ready)],
        PrState::Unblocked(UnblockedState::Draft) => vec![render_token(&config.draft)],
        PrState::NoPr => vec![render_token(&config.no_pull_request)],
        PrState::NotApplicable(NotApplicableState::Calculating) => {
            vec![render_token(&config.status_calculating)]
        }
        PrState::NotApplicable(_) | PrState::Unknown => vec![],
    };
    parts.join(" ")
}

fn render_blocked(blocked: BlockedState, config: &DisplayConfig) -> Vec<String> {
    let mut parts = Vec::new();
    if let Some(s) = blocked.branch_sync {
        parts.push(match s {
            BranchSyncState::Conflict => render_token(&config.conflict),
            BranchSyncState::UpdateBranch => render_token(&config.update_branch),
            BranchSyncState::SyncUnknown => render_token(&config.sync_unknown),
        });
    }
    if let Some(c) = blocked.ci {
        parts.push(match c {
            CiState::Fail => render_token(&config.ci_fail),
            CiState::ActionRequired => render_token(&config.ci_action),
            CiState::Pending => render_token(&config.ci_pending),
        });
    }
    if let Some(r) = blocked.review {
        parts.push(match r {
            ReviewState::ChangesRequested => render_token(&config.changes_requested),
            ReviewState::ReviewRequired => render_token(&config.review_required),
        });
    }
    if let Some(g) = blocked.generic {
        parts.push(match g {
            GenericBlockedState::BlockedUnknown => render_token(&config.blocked_unknown),
        });
    }
    parts
}

fn render_error(token: &ErrorToken, config: &DisplayConfig) -> String {
    render_error_token(&config.error, &token.message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contexts::evaluation::domain::pr_state::blocked::BlockedState;

    fn render_state(state: PrState) -> String {
        render_pr_state(state, &DisplayConfig::default())
    }

    #[test]
    fn no_pr_renders_create_pr() {
        assert_eq!(render_state(PrState::NoPr), "+ Create PR");
    }

    #[test]
    fn draft_renders_ready_for_review() {
        assert_eq!(
            render_state(PrState::Unblocked(UnblockedState::Draft)),
            "✎ Ready for review"
        );
    }

    #[test]
    fn review_required_renders_assign_reviewer() {
        let blocked = BlockedState {
            branch_sync: None,
            ci: None,
            review: Some(ReviewState::ReviewRequired),
            generic: None,
        };
        assert_eq!(render_state(PrState::Blocked(blocked)), "@ Assign reviewer");
    }

    #[test]
    fn ci_pending_renders_wait_for_ci() {
        let blocked = BlockedState {
            branch_sync: None,
            ci: Some(CiState::Pending),
            review: None,
            generic: None,
        };
        assert_eq!(render_state(PrState::Blocked(blocked)), "⧖ Wait for CI");
    }

    #[test]
    fn status_calculating_renders_wait_for_status() {
        assert_eq!(
            render_state(PrState::NotApplicable(NotApplicableState::Calculating)),
            "⧖ Wait for status"
        );
    }

    #[test]
    fn blocked_unknown_renders_check_merge_blocker() {
        let blocked = BlockedState {
            branch_sync: None,
            ci: None,
            review: None,
            generic: Some(GenericBlockedState::BlockedUnknown),
        };
        assert_eq!(
            render_state(PrState::Blocked(blocked)),
            "? Check merge blocker"
        );
    }

    #[test]
    fn multiple_blockers_are_joined_with_space() {
        let blocked = BlockedState {
            branch_sync: Some(BranchSyncState::Conflict),
            ci: Some(CiState::Fail),
            review: None,
            generic: None,
        };
        assert_eq!(
            render_state(PrState::Blocked(blocked)),
            "✗ Resolve conflict ✗ Fix CI failure"
        );
    }

    #[test]
    fn error_renders_with_message() {
        let config = DisplayConfig::default();
        let token = ErrorToken {
            message: "authentication required".to_owned(),
        };
        assert_eq!(render_error(&token, &config), "✗ authentication required");
    }
}
