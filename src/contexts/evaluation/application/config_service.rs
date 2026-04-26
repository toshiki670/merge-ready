use super::super::domain::display_config::{
    DisplayConfig, DisplayConfigRepository, TokenConfig, render_token,
};

pub struct ConfigService(DisplayConfig);

impl ConfigService {
    pub fn new(repo: &impl DisplayConfigRepository) -> Self {
        Self(repo.load())
    }

    pub fn render_merge_ready(&self) -> String {
        render_token(
            self.0.merge_ready.as_ref().unwrap_or(&default_token()),
            "✓",
            "merge-ready",
        )
    }

    pub fn render_conflict(&self) -> String {
        render_token(
            self.0.conflict.as_ref().unwrap_or(&default_token()),
            "✗",
            "conflict",
        )
    }

    pub fn render_update_branch(&self) -> String {
        render_token(
            self.0.update_branch.as_ref().unwrap_or(&default_token()),
            "✗",
            "update-branch",
        )
    }

    pub fn render_sync_unknown(&self) -> String {
        render_token(
            self.0.sync_unknown.as_ref().unwrap_or(&default_token()),
            "?",
            "sync-unknown",
        )
    }

    pub fn render_ci_fail(&self) -> String {
        render_token(
            self.0.ci_fail.as_ref().unwrap_or(&default_token()),
            "✗",
            "ci-fail",
        )
    }

    pub fn render_ci_action(&self) -> String {
        render_token(
            self.0.ci_action.as_ref().unwrap_or(&default_token()),
            "⚠",
            "ci-action",
        )
    }

    pub fn render_review(&self) -> String {
        render_token(
            self.0.review.as_ref().unwrap_or(&default_token()),
            "⚠",
            "review",
        )
    }

    pub fn render_auth_required(&self) -> String {
        render_token(
            self.0
                .error
                .as_ref()
                .and_then(|ec| ec.auth_required.as_ref())
                .unwrap_or(&default_token()),
            "!",
            "gh auth login",
        )
    }

    pub fn render_rate_limited(&self) -> String {
        render_token(
            self.0
                .error
                .as_ref()
                .and_then(|ec| ec.rate_limited.as_ref())
                .unwrap_or(&default_token()),
            "✗",
            "rate-limited",
        )
    }

    pub fn render_api_error(&self) -> String {
        render_token(
            self.0
                .error
                .as_ref()
                .and_then(|ec| ec.api_error.as_ref())
                .unwrap_or(&default_token()),
            "✗",
            "api-error",
        )
    }
}

pub fn default_display_config() -> DisplayConfig {
    let mut config = DisplayConfig::default();
    config.fill_defaults();
    config
}

fn default_token() -> TokenConfig {
    TokenConfig {
        symbol: None,
        label: None,
        format: None,
    }
}
