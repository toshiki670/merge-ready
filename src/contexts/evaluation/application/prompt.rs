use std::sync::Mutex;

use super::errors::{ErrorPresenter, ErrorToken, handle};
use super::port::ErrorLogger;
use crate::contexts::evaluation::domain::display_config::{
    DisplayConfigRepository, render_error_token, render_token,
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
    struct CapturingPresenter(Mutex<Option<ErrorToken>>);

    impl ErrorPresenter for CapturingPresenter {
        fn show_error(&self, token: ErrorToken) {
            *self
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(token);
        }
    }

    let config = config_repo.load();
    let presenter = CapturingPresenter(Mutex::new(None));

    let pr_state = match repo.fetch() {
        Ok(s) => s,
        Err(e) => {
            handle(e, logger, &presenter);
            let error = presenter
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take();
            let output = error.map_or(String::new(), |t| {
                render_error_token(&config.error, &t.message)
            });
            return (output, false);
        }
    };

    let is_terminal = pr_state.is_terminal();
    let parts = render_pr_state(pr_state, &config);
    (parts.join(" "), is_terminal)
}

fn render_pr_state(
    state: PrState,
    config: &crate::contexts::evaluation::domain::display_config::DisplayConfig,
) -> Vec<String> {
    match state {
        PrState::Blocked(blocked) => render_blocked(blocked, config),
        PrState::Unblocked(UnblockedState::MergeReady) => {
            vec![render_token(&config.merge_ready)]
        }
        PrState::Unblocked(UnblockedState::Draft) => vec![render_token(&config.draft)],
        PrState::NoPr => vec![render_token(&config.no_pull_request)],
        PrState::NotApplicable(NotApplicableState::Calculating) => {
            vec![render_token(&config.status_calculating)]
        }
        PrState::NotApplicable(_) | PrState::Unknown => vec![],
    }
}

fn render_blocked(
    blocked: BlockedState,
    config: &crate::contexts::evaluation::domain::display_config::DisplayConfig,
) -> Vec<String> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contexts::evaluation::domain::display_config::DisplayConfig;
    use crate::contexts::evaluation::domain::pr_state::blocked::BlockedState;

    fn render_state(state: PrState) -> String {
        render_pr_state(state, &DisplayConfig::default()).join(" ")
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
}
