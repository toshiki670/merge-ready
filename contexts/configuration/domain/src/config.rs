use serde::Deserialize;

const DEFAULT_FORMAT: &str = "$symbol $label";

#[derive(Deserialize, Default)]
pub struct Config {
    pub merge_ready: Option<TokenConfig>,
    pub conflict: Option<TokenConfig>,
    pub update_branch: Option<TokenConfig>,
    pub sync_unknown: Option<TokenConfig>,
    pub ci_fail: Option<TokenConfig>,
    pub ci_action: Option<TokenConfig>,
    pub review: Option<TokenConfig>,
    pub error: Option<ErrorConfig>,
}

#[derive(Deserialize, Default)]
pub struct TokenConfig {
    pub symbol: Option<String>,
    pub label: Option<String>,
    pub format: Option<String>,
}

impl TokenConfig {
    pub fn render(&self, default_symbol: &str, default_label: &str) -> String {
        let symbol = self.symbol.as_deref().unwrap_or(default_symbol);
        let label = self.label.as_deref().unwrap_or(default_label);
        let fmt = self.format.as_deref().unwrap_or(DEFAULT_FORMAT);
        fmt.replace("$symbol", symbol).replace("$label", label)
    }
}

#[derive(Deserialize, Default)]
pub struct ErrorConfig {
    pub auth_required: Option<TokenConfig>,
    pub rate_limited: Option<TokenConfig>,
    pub api_error: Option<TokenConfig>,
}
