use serde::Serialize;

const DEFAULT_FORMAT: &str = "$symbol $label";
const DEFAULT_ERROR_FORMAT: &str = "$symbol $message";

#[derive(Serialize)]
pub struct DisplayConfig {
    pub merge_ready: TokenConfig,
    pub no_pull_request: TokenConfig,
    pub conflict: TokenConfig,
    pub update_branch: TokenConfig,
    pub sync_unknown: TokenConfig,
    pub ci_fail: TokenConfig,
    pub ci_action: TokenConfig,
    pub ci_pending: TokenConfig,
    pub changes_requested: TokenConfig,
    pub review_required: TokenConfig,
    pub draft: TokenConfig,
    pub status_calculating: TokenConfig,
    pub error: ErrorConfig,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        let tok = |symbol: &str, label: &str| TokenConfig {
            symbol: symbol.to_owned(),
            label: label.to_owned(),
            format: DEFAULT_FORMAT.to_owned(),
        };
        Self {
            merge_ready: tok("✓", "Ready for merge"),
            no_pull_request: tok("+", "Create PR"),
            conflict: tok("✗", "Resolve conflict"),
            update_branch: tok("✗", "Update branch"),
            sync_unknown: tok("?", "Check branch sync"),
            ci_fail: tok("✗", "Fix CI failure"),
            ci_action: tok("⚠", "Run CI action"),
            ci_pending: tok("⧖", "Wait for CI"),
            changes_requested: tok("⚠", "Resolve review"),
            review_required: tok("@", "Assign reviewer"),
            draft: tok("✎", "Ready for review"),
            status_calculating: tok("⧖", "Wait for status"),
            error: ErrorConfig::default(),
        }
    }
}

pub trait DisplayConfigRepository {
    fn load(&self) -> DisplayConfig;
}

#[derive(Serialize)]
pub struct TokenConfig {
    pub symbol: String,
    pub label: String,
    pub format: String,
}

#[must_use]
pub fn render_token(token: &TokenConfig) -> String {
    token
        .format
        .replace("$symbol", &token.symbol)
        .replace("$label", &token.label)
}

#[derive(Serialize)]
pub struct ErrorConfig {
    pub symbol: String,
    pub format: String,
}

impl Default for ErrorConfig {
    fn default() -> Self {
        Self {
            symbol: "✗".to_owned(),
            format: DEFAULT_ERROR_FORMAT.to_owned(),
        }
    }
}

#[must_use]
pub fn render_error_token(config: &ErrorConfig, message: &str) -> String {
    config
        .format
        .replace("$symbol", &config.symbol)
        .replace("$message", message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_sets_no_pull_request() {
        let config = DisplayConfig::default();
        assert_eq!(config.no_pull_request.symbol, "+");
        assert_eq!(config.no_pull_request.label, "Create PR");
    }

    #[test]
    fn default_sets_draft() {
        let config = DisplayConfig::default();
        assert_eq!(config.draft.symbol, "✎");
        assert_eq!(config.draft.label, "Ready for review");
    }

    #[test]
    fn default_sets_review_required() {
        let config = DisplayConfig::default();
        assert_eq!(config.review_required.symbol, "@");
        assert_eq!(config.review_required.label, "Assign reviewer");
    }

    #[test]
    fn default_sets_ci_pending() {
        let config = DisplayConfig::default();
        assert_eq!(config.ci_pending.symbol, "⧖");
        assert_eq!(config.ci_pending.label, "Wait for CI");
    }

    #[test]
    fn default_sets_status_calculating() {
        let config = DisplayConfig::default();
        assert_eq!(config.status_calculating.symbol, "⧖");
        assert_eq!(config.status_calculating.label, "Wait for status");
    }

    #[test]
    fn default_error_config_sets_symbol_and_format() {
        let config = DisplayConfig::default();
        assert_eq!(config.error.symbol, "✗");
        assert_eq!(config.error.format, "$symbol $message");
    }

    #[test]
    fn render_error_token_substitutes_symbol_and_message() {
        let config = ErrorConfig::default();
        assert_eq!(
            render_error_token(&config, "rate limited"),
            "✗ rate limited"
        );
    }

    #[test]
    fn render_error_token_respects_custom_format() {
        let config = ErrorConfig {
            symbol: "!".to_owned(),
            format: "[$symbol] $message".to_owned(),
        };
        assert_eq!(
            render_error_token(&config, "authentication required"),
            "[!] authentication required"
        );
    }
}
