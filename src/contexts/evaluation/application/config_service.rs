use super::super::domain::display_config::{
    DisplayConfig, DisplayConfigRepository, TokenConfig, render_token as apply_format,
};
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
            .map(|t| render_token(&config, t))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

pub fn default_display_config() -> DisplayConfig {
    let mut config = DisplayConfig::default();
    config.fill_defaults();
    config
}

fn render_token(config: &DisplayConfig, token: &OutputToken) -> String {
    match token {
        OutputToken::MergeReady => apply_format(
            config.merge_ready.as_ref().unwrap_or(&empty()),
            "✓",
            "merge-ready",
        ),
        OutputToken::NoPullRequest => apply_format(
            config.no_pull_request.as_ref().unwrap_or(&empty()),
            "+",
            "create-pr",
        ),
        OutputToken::Conflict => apply_format(
            config.conflict.as_ref().unwrap_or(&empty()),
            "✗",
            "conflict",
        ),
        OutputToken::UpdateBranch => apply_format(
            config.update_branch.as_ref().unwrap_or(&empty()),
            "✗",
            "update-branch",
        ),
        OutputToken::SyncUnknown => apply_format(
            config.sync_unknown.as_ref().unwrap_or(&empty()),
            "?",
            "sync-unknown",
        ),
        OutputToken::CiFail => {
            apply_format(config.ci_fail.as_ref().unwrap_or(&empty()), "✗", "ci-fail")
        }
        OutputToken::CiAction => apply_format(
            config.ci_action.as_ref().unwrap_or(&empty()),
            "⚠",
            "ci-action",
        ),
        OutputToken::ReviewRequested => {
            apply_format(config.review.as_ref().unwrap_or(&empty()), "⚠", "review")
        }
    }
}

fn render_error(config: &DisplayConfig, token: ErrorToken) -> String {
    let ec = config.error.as_ref();
    match token {
        ErrorToken::AuthRequired => apply_format(
            ec.and_then(|e| e.auth_required.as_ref())
                .unwrap_or(&empty()),
            "!",
            "gh auth login",
        ),
        ErrorToken::RateLimited => apply_format(
            ec.and_then(|e| e.rate_limited.as_ref()).unwrap_or(&empty()),
            "✗",
            "rate-limited",
        ),
        ErrorToken::ApiError => apply_format(
            ec.and_then(|e| e.api_error.as_ref()).unwrap_or(&empty()),
            "✗",
            "api-error",
        ),
    }
}

fn empty() -> TokenConfig {
    TokenConfig {
        symbol: None,
        label: None,
        format: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DefaultRepo;
    impl DisplayConfigRepository for DefaultRepo {
        fn load(&self) -> DisplayConfig {
            default_display_config()
        }
    }

    #[test]
    fn no_pull_request_renders_with_default_config() {
        let result = render_output(&[OutputToken::NoPullRequest], None, &DefaultRepo);
        assert_eq!(result, "+ create-pr");
    }
}
