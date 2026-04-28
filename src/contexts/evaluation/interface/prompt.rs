use crate::contexts::evaluation::application::config_service;
use crate::contexts::evaluation::application::errors::ErrorToken;
use crate::contexts::evaluation::application::port::ErrorLogger;
use crate::contexts::evaluation::application::prompt::display_item::DisplayItem;
use crate::contexts::evaluation::application::prompt::fetch;
use crate::contexts::evaluation::domain::display_config::{
    DisplayConfig, DisplayConfigRepository, TokenConfig, render_error_token, render_token,
};
use crate::contexts::evaluation::domain::pr_state::PrRepository;

pub fn render<R, C, L>(repo: &R, config_repo: &C, logger: &L) -> (String, bool)
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
            (output, is_terminal)
        }
        Err(token) => (render_error(&token, &config), false),
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

    fn render_item(item: DisplayItem) -> String {
        let config = DisplayConfig::default();
        render_token(item_to_token(&item, &config))
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
