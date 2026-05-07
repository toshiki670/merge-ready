use super::{DisplayConfig, ErrorConfig, TokenConfig};

/// PR 状態トークン 12 種のデフォルトフォーマット。`( #$pr_id)` は複数 PR 時のみ展開される。
const DEFAULT_PR_FORMAT: &str = "$symbol $label( #$pr_id)";
/// `no_pull_request` / `error` トークンのデフォルトフォーマット。
const DEFAULT_FORMAT: &str = "$symbol $label";
const DEFAULT_ERROR_FORMAT: &str = "$symbol $message";

impl Default for DisplayConfig {
    fn default() -> Self {
        let pr_tok = |symbol: &str, label: &str| TokenConfig {
            symbol: symbol.to_owned(),
            label: label.to_owned(),
            format: DEFAULT_PR_FORMAT.to_owned(),
        };
        let tok = |symbol: &str, label: &str| TokenConfig {
            symbol: symbol.to_owned(),
            label: label.to_owned(),
            format: DEFAULT_FORMAT.to_owned(),
        };
        Self {
            merge_ready: pr_tok("✓", "Ready for merge"),
            no_pull_request: tok("+", "Create PR"),
            conflict: pr_tok("✗", "Resolve conflict"),
            update_branch: pr_tok("✗", "Update branch"),
            sync_unknown: pr_tok("?", "Check branch sync"),
            ci_fail: pr_tok("✗", "Fix CI failure"),
            ci_action: pr_tok("⚠", "Run CI action"),
            ci_pending: pr_tok("⧖", "Wait for CI"),
            changes_requested: pr_tok("⚠", "Resolve review"),
            review_required: pr_tok("@", "Assign reviewer"),
            draft: pr_tok("✎", "Ready for review"),
            status_calculating: pr_tok("⧖", "Wait for status"),
            blocked_unknown: pr_tok("?", "Check merge blocker"),
            error: ErrorConfig::default(),
        }
    }
}

impl Default for ErrorConfig {
    fn default() -> Self {
        Self {
            symbol: "✗".to_owned(),
            format: DEFAULT_ERROR_FORMAT.to_owned(),
        }
    }
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
    fn default_sets_blocked_unknown() {
        let config = DisplayConfig::default();
        assert_eq!(config.blocked_unknown.symbol, "?");
        assert_eq!(config.blocked_unknown.label, "Check merge blocker");
    }

    #[test]
    fn default_error_config_sets_symbol_and_format() {
        let config = DisplayConfig::default();
        assert_eq!(config.error.symbol, "✗");
        assert_eq!(config.error.format, "$symbol $message");
    }

    #[test]
    fn default_toml_contains_stable_sections() {
        let toml = toml::to_string_pretty(&DisplayConfig::default()).unwrap();
        assert!(toml.contains("[merge_ready]"));
        assert!(toml.contains("[no_pull_request]"));
        assert!(toml.contains("[error]"));
        assert!(toml.contains("format = \"$symbol $label( #$pr_id)\""));
    }
}
