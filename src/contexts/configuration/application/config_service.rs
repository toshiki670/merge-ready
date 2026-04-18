use super::super::domain::config::{Config, TokenConfig};
use super::super::domain::repository::ConfigRepository;

pub struct ConfigService(Config);

impl ConfigService {
    pub fn new(repo: &impl ConfigRepository) -> Self {
        Self(repo.load())
    }

    pub fn render_merge_ready(&self) -> String {
        self.0
            .merge_ready
            .as_ref()
            .unwrap_or(&default_token())
            .render("✓", "merge-ready")
    }

    pub fn render_conflict(&self) -> String {
        self.0
            .conflict
            .as_ref()
            .unwrap_or(&default_token())
            .render("✗", "conflict")
    }

    pub fn render_update_branch(&self) -> String {
        self.0
            .update_branch
            .as_ref()
            .unwrap_or(&default_token())
            .render("✗", "update-branch")
    }

    pub fn render_sync_unknown(&self) -> String {
        self.0
            .sync_unknown
            .as_ref()
            .unwrap_or(&default_token())
            .render("?", "sync-unknown")
    }

    pub fn render_ci_fail(&self) -> String {
        self.0
            .ci_fail
            .as_ref()
            .unwrap_or(&default_token())
            .render("✗", "ci-fail")
    }

    pub fn render_ci_action(&self) -> String {
        self.0
            .ci_action
            .as_ref()
            .unwrap_or(&default_token())
            .render("⚠", "ci-action")
    }

    pub fn render_review(&self) -> String {
        self.0
            .review
            .as_ref()
            .unwrap_or(&default_token())
            .render("⚠", "review")
    }

    pub fn render_auth_required(&self) -> String {
        self.0
            .error
            .as_ref()
            .and_then(|ec| ec.auth_required.as_ref())
            .unwrap_or(&default_token())
            .render("!", "gh auth login")
    }

    pub fn render_rate_limited(&self) -> String {
        self.0
            .error
            .as_ref()
            .and_then(|ec| ec.rate_limited.as_ref())
            .unwrap_or(&default_token())
            .render("✗", "rate-limited")
    }

    pub fn render_api_error(&self) -> String {
        self.0
            .error
            .as_ref()
            .and_then(|ec| ec.api_error.as_ref())
            .unwrap_or(&default_token())
            .render("✗", "api-error")
    }
}

fn default_token() -> TokenConfig {
    TokenConfig::default()
}
