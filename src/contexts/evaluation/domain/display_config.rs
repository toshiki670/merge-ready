use serde::{Deserialize, Serialize};

const DEFAULT_FORMAT: &str = "$symbol $label";

#[derive(Deserialize, Serialize, Default)]
pub struct DisplayConfig {
    pub merge_ready: Option<TokenConfig>,
    pub no_pull_request: Option<TokenConfig>,
    pub conflict: Option<TokenConfig>,
    pub update_branch: Option<TokenConfig>,
    pub sync_unknown: Option<TokenConfig>,
    pub ci_fail: Option<TokenConfig>,
    pub ci_action: Option<TokenConfig>,
    pub ci_pending: Option<TokenConfig>,
    pub review: Option<TokenConfig>,
    pub review_required: Option<TokenConfig>,
    pub draft: Option<TokenConfig>,
    pub error: Option<ErrorConfig>,
}

impl DisplayConfig {
    pub fn fill_defaults(&mut self) {
        let tok = |symbol: &str, label: &str| TokenConfig {
            symbol: Some(symbol.to_owned()),
            label: Some(label.to_owned()),
            format: None,
        };
        self.merge_ready
            .get_or_insert_with(|| tok("✓", "merge-ready"));
        self.no_pull_request
            .get_or_insert_with(|| tok("+", "create-pr"));
        self.conflict.get_or_insert_with(|| tok("✗", "conflict"));
        self.update_branch
            .get_or_insert_with(|| tok("✗", "update-branch"));
        self.sync_unknown
            .get_or_insert_with(|| tok("?", "sync-unknown"));
        self.ci_fail.get_or_insert_with(|| tok("✗", "ci-fail"));
        self.ci_action.get_or_insert_with(|| tok("⚠", "ci-action"));
        self.ci_pending
            .get_or_insert_with(|| tok("⧖", "wait-for-ci"));
        self.review.get_or_insert_with(|| tok("⚠", "review"));
        self.review_required
            .get_or_insert_with(|| tok("@", "assign-reviewer"));
        self.draft
            .get_or_insert_with(|| tok("✎", "ready-for-review"));
        let error = self.error.get_or_insert_with(ErrorConfig::empty);
        error
            .auth_required
            .get_or_insert_with(|| tok("!", "gh auth login"));
        error
            .rate_limited
            .get_or_insert_with(|| tok("✗", "rate-limited"));
        error.api_error.get_or_insert_with(|| tok("✗", "api-error"));
    }
}

pub trait DisplayConfigRepository {
    fn load(&self) -> DisplayConfig;
}

#[derive(Deserialize, Serialize)]
pub struct TokenConfig {
    pub symbol: Option<String>,
    pub label: Option<String>,
    pub format: Option<String>,
}

#[must_use]
pub fn render_token(token: &TokenConfig, default_symbol: &str, default_label: &str) -> String {
    let symbol = token.symbol.as_deref().unwrap_or(default_symbol);
    let label = token.label.as_deref().unwrap_or(default_label);
    let fmt = token.format.as_deref().unwrap_or(DEFAULT_FORMAT);
    fmt.replace("$symbol", symbol).replace("$label", label)
}

#[derive(Deserialize, Serialize)]
pub struct ErrorConfig {
    pub auth_required: Option<TokenConfig>,
    pub rate_limited: Option<TokenConfig>,
    pub api_error: Option<TokenConfig>,
}

impl ErrorConfig {
    fn empty() -> Self {
        Self {
            auth_required: None,
            rate_limited: None,
            api_error: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_defaults_sets_no_pull_request() {
        let mut config = DisplayConfig::default();
        config.fill_defaults();
        let tok = config.no_pull_request.as_ref().unwrap();
        assert_eq!(tok.symbol.as_deref(), Some("+"));
        assert_eq!(tok.label.as_deref(), Some("create-pr"));
    }

    #[test]
    fn fill_defaults_sets_draft() {
        let mut config = DisplayConfig::default();
        config.fill_defaults();
        let tok = config.draft.as_ref().unwrap();
        assert_eq!(tok.symbol.as_deref(), Some("✎"));
        assert_eq!(tok.label.as_deref(), Some("ready-for-review"));
    }

    #[test]
    fn fill_defaults_sets_review_required() {
        let mut config = DisplayConfig::default();
        config.fill_defaults();
        let tok = config.review_required.as_ref().unwrap();
        assert_eq!(tok.symbol.as_deref(), Some("@"));
        assert_eq!(tok.label.as_deref(), Some("assign-reviewer"));
    }

    #[test]
    fn fill_defaults_sets_ci_pending() {
        let mut config = DisplayConfig::default();
        config.fill_defaults();
        let tok = config.ci_pending.as_ref().unwrap();
        assert_eq!(tok.symbol.as_deref(), Some("⧖"));
        assert_eq!(tok.label.as_deref(), Some("wait-for-ci"));
    }
}
