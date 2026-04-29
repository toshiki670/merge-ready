use crate::contexts::evaluation::application::config_service;
use crate::contexts::evaluation::application::errors::ErrorToken;
use crate::contexts::evaluation::application::port::ErrorLogger;
use crate::contexts::evaluation::application::prompt::display_item::DisplayItem;
use crate::contexts::evaluation::application::prompt::fetch;
use crate::contexts::evaluation::domain::display_config::{
    DisplayConfig, DisplayConfigRepository, TokenConfig, render_error_token, render_token,
};
use crate::contexts::evaluation::domain::pr_state::PrRepository;

/// daemon のキャッシュ更新頻度を制御するヒント。
/// evaluation ドメインの知識（CI 状態・終端状態）を daemon に伝える interface 層の出力型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheHint {
    /// CI 実行中。素早いリフレッシュが必要。
    Hot,
    /// CI 完了・通常監視中。
    Warm,
    /// PR が merged / closed。リフレッシュ不要。
    Terminal,
}

pub fn render<R, C, L>(repo: &R, config_repo: &C, logger: &L) -> (String, CacheHint)
where
    R: PrRepository,
    C: DisplayConfigRepository,
    L: ErrorLogger,
{
    let config = config_service::load(config_repo);
    match fetch(repo, logger) {
        Ok((items, is_terminal)) => {
            let output = items
                .iter()
                .map(|item| render_token(item_to_token(item, &config)))
                .collect::<Vec<_>>()
                .join(" ");
            let hint = if is_terminal {
                CacheHint::Terminal
            } else if items.iter().any(|i| matches!(i, DisplayItem::CiPending)) {
                CacheHint::Hot
            } else {
                CacheHint::Warm
            };
            (output, hint)
        }
        Err(token) => (render_error(&token, &config), CacheHint::Warm),
    }
}

fn item_to_token<'a>(item: &DisplayItem, config: &'a DisplayConfig) -> &'a TokenConfig {
    match item {
        DisplayItem::MergeReady => &config.merge_ready,
        DisplayItem::NoPullRequest => &config.no_pull_request,
        DisplayItem::Conflict => &config.conflict,
        DisplayItem::UpdateBranch => &config.update_branch,
        DisplayItem::SyncUnknown => &config.sync_unknown,
        DisplayItem::CiFail => &config.ci_fail,
        DisplayItem::CiAction => &config.ci_action,
        DisplayItem::CiPending => &config.ci_pending,
        DisplayItem::ChangesRequested => &config.changes_requested,
        DisplayItem::ReviewRequired => &config.review_required,
        DisplayItem::Draft => &config.draft,
        DisplayItem::StatusCalculating => &config.status_calculating,
        DisplayItem::BlockedUnknown => &config.blocked_unknown,
    }
}

fn render_error(token: &ErrorToken, config: &DisplayConfig) -> String {
    render_error_token(&config.error, &token.message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contexts::evaluation::application::prompt::display_item::DisplayItem;
    use crate::contexts::evaluation::domain::error::RepositoryError;
    use crate::contexts::evaluation::domain::pr_state::{PrRepository, PrState};

    fn render_item(item: DisplayItem) -> String {
        let config = DisplayConfig::default();
        render_token(item_to_token(&item, &config))
    }

    struct StubRepo(PrState);
    impl PrRepository for StubRepo {
        fn fetch(&self) -> Result<PrState, RepositoryError> {
            Ok(self.0)
        }
    }

    struct ErrRepo;
    impl PrRepository for ErrRepo {
        fn fetch(&self) -> Result<PrState, RepositoryError> {
            Err(RepositoryError::Unexpected)
        }
    }

    struct NoOpLogger;
    impl crate::contexts::evaluation::application::port::ErrorLogger for NoOpLogger {
        fn log(&self, _: &crate::contexts::evaluation::application::port::LogRecord) {}
    }

    struct NoOpConfigRepo;
    impl DisplayConfigRepository for NoOpConfigRepo {
        fn load(&self) -> DisplayConfig {
            DisplayConfig::default()
        }
    }

    fn hint_for(state: PrState) -> CacheHint {
        let (_, hint) = render(&StubRepo(state), &NoOpConfigRepo, &NoOpLogger);
        hint
    }

    // ── CacheHint 導出 ──────────────────────────────────────────────────────

    #[test]
    fn ci_pending_returns_hot() {
        use crate::contexts::evaluation::domain::pr_state::blocked::BlockedState;
        use crate::contexts::evaluation::domain::pr_state::blocked::ci::CiState;
        let state = PrState::Blocked(BlockedState {
            branch_sync: None,
            ci: Some(CiState::Pending),
            review: None,
            generic: None,
        });
        assert_eq!(hint_for(state), CacheHint::Hot);
    }

    #[test]
    fn ci_fail_returns_warm() {
        use crate::contexts::evaluation::domain::pr_state::blocked::BlockedState;
        use crate::contexts::evaluation::domain::pr_state::blocked::ci::CiState;
        let state = PrState::Blocked(BlockedState {
            branch_sync: None,
            ci: Some(CiState::Fail),
            review: None,
            generic: None,
        });
        assert_eq!(hint_for(state), CacheHint::Warm);
    }

    #[test]
    fn merge_ready_returns_warm() {
        use crate::contexts::evaluation::domain::pr_state::unblocked::UnblockedState;
        assert_eq!(
            hint_for(PrState::Unblocked(UnblockedState::MergeReady)),
            CacheHint::Warm
        );
    }

    #[test]
    fn merged_pr_returns_terminal() {
        use crate::contexts::evaluation::domain::pr_state::NotApplicableState;
        assert_eq!(
            hint_for(PrState::NotApplicable(NotApplicableState::Merged)),
            CacheHint::Terminal
        );
    }

    #[test]
    fn closed_pr_returns_terminal() {
        use crate::contexts::evaluation::domain::pr_state::NotApplicableState;
        assert_eq!(
            hint_for(PrState::NotApplicable(NotApplicableState::Closed)),
            CacheHint::Terminal
        );
    }

    #[test]
    fn fetch_error_returns_warm() {
        let (_, hint) = render(&ErrRepo, &NoOpConfigRepo, &NoOpLogger);
        assert_eq!(hint, CacheHint::Warm);
    }

    #[test]
    fn no_pull_request_renders_create_pr() {
        assert_eq!(render_item(DisplayItem::NoPullRequest), "+ Create PR");
    }

    #[test]
    fn draft_renders_ready_for_review() {
        assert_eq!(render_item(DisplayItem::Draft), "✎ Ready for review");
    }

    #[test]
    fn review_required_renders_assign_reviewer() {
        assert_eq!(
            render_item(DisplayItem::ReviewRequired),
            "@ Assign reviewer"
        );
    }

    #[test]
    fn ci_pending_renders_wait_for_ci() {
        assert_eq!(render_item(DisplayItem::CiPending), "⧖ Wait for CI");
    }

    #[test]
    fn status_calculating_renders_wait_for_status() {
        assert_eq!(
            render_item(DisplayItem::StatusCalculating),
            "⧖ Wait for status"
        );
    }

    #[test]
    fn blocked_unknown_renders_check_merge_blocker() {
        assert_eq!(
            render_item(DisplayItem::BlockedUnknown),
            "? Check merge blocker"
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
