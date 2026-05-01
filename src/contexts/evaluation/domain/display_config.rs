use serde::Serialize;

use super::format_parser::{Segment, parse_segments};
use super::style_spec::StyleSpec;

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
    pub blocked_unknown: TokenConfig,
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
            blocked_unknown: tok("?", "Check merge blocker"),
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
    let substituted = token
        .format
        .replace("$symbol", &token.symbol)
        .replace("$label", &token.label);
    render_segments(&substituted)
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
    let substituted = config
        .format
        .replace("$symbol", &config.symbol)
        .replace("$message", message);
    render_segments(&substituted)
}

fn render_segments(s: &str) -> String {
    parse_segments(s)
        .into_iter()
        .map(|seg| match seg {
            Segment::Text(t) => t,
            Segment::Styled { content, style_str } => StyleSpec::parse(&style_str)
                .to_ansi_style()
                .paint(content)
                .to_string(),
        })
        .collect()
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

    // ── スタイル構文のテスト ─────────────────────────────────────────────────

    #[test]
    fn render_token_plain_format_unaffected() {
        let tok = TokenConfig {
            symbol: "✓".to_owned(),
            label: "Ready".to_owned(),
            format: "$symbol $label".to_owned(),
        };
        assert_eq!(render_token(&tok), "✓ Ready");
    }

    #[test]
    fn render_token_styled_contains_ansi() {
        let tok = TokenConfig {
            symbol: "✓".to_owned(),
            label: "Ready".to_owned(),
            format: "[$symbol](bold green) $label".to_owned(),
        };
        let out = render_token(&tok);
        assert!(out.contains("\x1b["), "expected ANSI codes in: {out:?}");
        assert!(out.contains("✓"));
        assert!(out.contains("Ready"));
    }

    #[test]
    fn render_token_placeholder_substituted_before_style() {
        let tok = TokenConfig {
            symbol: "✓".to_owned(),
            label: "Ready".to_owned(),
            format: "[$symbol $label](green)".to_owned(),
        };
        let out = render_token(&tok);
        assert!(
            out.contains("✓ Ready"),
            "placeholder must be substituted: {out:?}"
        );
    }

    #[test]
    fn render_error_token_styled_contains_ansi() {
        let config = ErrorConfig {
            symbol: "✗".to_owned(),
            format: "[$symbol](bold red) $message".to_owned(),
        };
        let out = render_error_token(&config, "failed");
        assert!(out.contains("\x1b["), "expected ANSI codes in: {out:?}");
        assert!(out.contains("✗"));
        assert!(out.contains("failed"));
    }

    #[test]
    fn render_error_token_plain_format_unaffected() {
        let config = ErrorConfig::default();
        assert_eq!(render_error_token(&config, "oops"), "✗ oops");
    }

    #[test]
    fn render_token_text_after_style_is_reset() {
        // `[$symbol](bold green) $label` のとき、$label はスタイルを引き継がない。
        // nu-ansi-term は styled 部分の末尾に reset (\x1b[0m) を挿入するため
        // それ以降の文字はデフォルトカラーになる。
        let tok = TokenConfig {
            symbol: "✓".to_owned(),
            label: "Ready".to_owned(),
            format: "[$symbol](bold green) $label".to_owned(),
        };
        let out = render_token(&tok);
        let reset = "\x1b[0m";
        let reset_pos = out
            .find(reset)
            .expect("reset sequence must exist after styled segment");
        let label_pos = out.find("Ready").expect("label must exist in output");
        assert!(
            reset_pos < label_pos,
            "reset must appear before the plain-text label: {out:?}"
        );
        let after_reset = &out[reset_pos + reset.len()..];
        assert!(
            !after_reset.contains("\x1b["),
            "no ANSI codes should follow the reset: {out:?}"
        );
    }

    #[test]
    fn render_token_plain_format_identical_to_simple_replace() {
        // 後方互換: スタイル構文なしの format は単純置換と完全一致する。
        let tok = TokenConfig {
            symbol: "✓".to_owned(),
            label: "Ready for merge".to_owned(),
            format: "$symbol $label".to_owned(),
        };
        let expected = tok
            .format
            .replace("$symbol", &tok.symbol)
            .replace("$label", &tok.label);
        assert_eq!(render_token(&tok), expected);
    }
}
