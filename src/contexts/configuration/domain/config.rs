use serde::{Deserialize, Serialize};

const DEFAULT_FORMAT: &str = "$symbol $label";

pub const CURRENT_VERSION: u32 = 1;

#[derive(Deserialize, Serialize, Default)]
pub struct Config {
    #[serde(default)]
    pub version: u32,
    pub merge_ready: Option<TokenConfig>,
    pub conflict: Option<TokenConfig>,
    pub update_branch: Option<TokenConfig>,
    pub sync_unknown: Option<TokenConfig>,
    pub ci_fail: Option<TokenConfig>,
    pub ci_action: Option<TokenConfig>,
    pub review: Option<TokenConfig>,
    pub error: Option<ErrorConfig>,
}

impl Config {
    pub fn fill_defaults(&mut self) {
        let tok = |symbol: &str, label: &str| TokenConfig {
            symbol: Some(symbol.to_owned()),
            label: Some(label.to_owned()),
            format: None,
        };
        self.merge_ready
            .get_or_insert_with(|| tok("✓", "merge-ready"));
        self.conflict.get_or_insert_with(|| tok("✗", "conflict"));
        self.update_branch
            .get_or_insert_with(|| tok("✗", "update-branch"));
        self.sync_unknown
            .get_or_insert_with(|| tok("?", "sync-unknown"));
        self.ci_fail.get_or_insert_with(|| tok("✗", "ci-fail"));
        self.ci_action.get_or_insert_with(|| tok("⚠", "ci-action"));
        self.review.get_or_insert_with(|| tok("⚠", "review"));
        let error = self.error.get_or_insert_with(ErrorConfig::default);
        error
            .auth_required
            .get_or_insert_with(|| tok("!", "gh auth login"));
        error
            .rate_limited
            .get_or_insert_with(|| tok("✗", "rate-limited"));
        error.api_error.get_or_insert_with(|| tok("✗", "api-error"));
    }
}

#[derive(Deserialize, Serialize, Default)]
pub struct TokenConfig {
    pub symbol: Option<String>,
    pub label: Option<String>,
    pub format: Option<String>,
}

impl TokenConfig {
    #[must_use]
    pub fn render(&self, default_symbol: &str, default_label: &str) -> String {
        let symbol = self.symbol.as_deref().unwrap_or(default_symbol);
        let label = self.label.as_deref().unwrap_or(default_label);
        let fmt = self.format.as_deref().unwrap_or(DEFAULT_FORMAT);
        fmt.replace("$symbol", symbol).replace("$label", label)
    }
}

#[derive(Deserialize, Serialize, Default)]
pub struct ErrorConfig {
    pub auth_required: Option<TokenConfig>,
    pub rate_limited: Option<TokenConfig>,
    pub api_error: Option<TokenConfig>,
}
