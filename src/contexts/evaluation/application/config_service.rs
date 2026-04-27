use super::super::domain::display_config::{DisplayConfig, DisplayConfigRepository, render_token};
use super::{OutputToken, errors::ErrorToken};

pub fn render_output(
    tokens: &[OutputToken],
    error: Option<ErrorToken>,
    repo: &impl DisplayConfigRepository,
) -> String {
    let config = repo.load();
    if let Some(err) = error {
        render_error(&config, err)
    } else {
        tokens
            .iter()
            .map(|t| render_token_output(&config, t))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

pub fn default_display_config() -> DisplayConfig {
    DisplayConfig::default()
}

fn render_token_output(config: &DisplayConfig, token: &OutputToken) -> String {
    match token {
        OutputToken::MergeReady => render_token(&config.merge_ready),
        OutputToken::NoPullRequest => render_token(&config.no_pull_request),
        OutputToken::Conflict => render_token(&config.conflict),
        OutputToken::UpdateBranch => render_token(&config.update_branch),
        OutputToken::SyncUnknown => render_token(&config.sync_unknown),
        OutputToken::CiFail => render_token(&config.ci_fail),
        OutputToken::CiAction => render_token(&config.ci_action),
        OutputToken::CiPending => render_token(&config.ci_pending),
        OutputToken::ReviewRequested => render_token(&config.changes_requested),
        OutputToken::ReviewRequired => render_token(&config.review_required),
        OutputToken::Draft => render_token(&config.draft),
        OutputToken::StatusCalculating => render_token(&config.status_calculating),
    }
}

fn render_error(config: &DisplayConfig, token: ErrorToken) -> String {
    match token {
        ErrorToken::AuthRequired => render_token(&config.error.auth_required),
        ErrorToken::RateLimited => render_token(&config.error.rate_limited),
        ErrorToken::ApiError => render_token(&config.error.api_error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DefaultRepo;
    impl DisplayConfigRepository for DefaultRepo {
        fn load(&self) -> DisplayConfig {
            DisplayConfig::default()
        }
    }

    #[test]
    fn no_pull_request_renders_with_default_config() {
        let result = render_output(&[OutputToken::NoPullRequest], None, &DefaultRepo);
        assert_eq!(result, "+ Create PR");
    }

    #[test]
    fn draft_renders_with_default_config() {
        let result = render_output(&[OutputToken::Draft], None, &DefaultRepo);
        assert_eq!(result, "✎ Ready for review");
    }

    #[test]
    fn review_required_renders_with_default_config() {
        let result = render_output(&[OutputToken::ReviewRequired], None, &DefaultRepo);
        assert_eq!(result, "@ Assign reviewer");
    }

    #[test]
    fn ci_pending_renders_with_default_config() {
        let result = render_output(&[OutputToken::CiPending], None, &DefaultRepo);
        assert_eq!(result, "⧖ Wait for CI");
    }

    #[test]
    fn status_calculating_renders_with_default_config() {
        let result = render_output(&[OutputToken::StatusCalculating], None, &DefaultRepo);
        assert_eq!(result, "⧖ Wait for status");
    }
}
