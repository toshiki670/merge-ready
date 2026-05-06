use serde::Serialize;

use super::format_parser::{Segment, parse_segments};
use super::style_spec::StyleSpec;

/// PR 状態トークン 12 種のデフォルトフォーマット。`( #$pr_id)` は複数 PR 時のみ展開される。
const DEFAULT_PR_FORMAT: &str = "$symbol $label( #$pr_id)";
/// `no_pull_request` / `error` トークンのデフォルトフォーマット。
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

pub trait DisplayConfigRepository {
    fn load(&self) -> DisplayConfig;
}

#[derive(Serialize)]
pub struct TokenConfig {
    pub symbol: String,
    pub label: String,
    pub format: String,
}

/// `pr_id` が `Some` の場合は `$pr_id` を置換する。`None` の場合は `$pr_id` を literal のまま残す。
/// `(...)` ブロック内の変数がすべて空の場合、そのブロックは出力されない。
#[must_use]
pub fn render_token(token: &TokenConfig, pr_id: Option<&str>) -> String {
    let mut vars: Vec<(&str, &str)> = vec![("symbol", &token.symbol), ("label", &token.label)];
    if let Some(id) = pr_id {
        vars.push(("pr_id", id));
    }
    render_with_vars(&token.format, &vars)
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
    let vars: Vec<(&str, &str)> = vec![("symbol", &config.symbol), ("message", message)];
    render_with_vars(&config.format, &vars)
}

fn render_with_vars(format: &str, vars: &[(&str, &str)]) -> String {
    eval_segments(&parse_segments(format), vars)
}

fn eval_segments(segs: &[Segment], vars: &[(&str, &str)]) -> String {
    segs.iter().map(|s| eval_segment(s, vars)).collect()
}

fn eval_segment(seg: &Segment, vars: &[(&str, &str)]) -> String {
    match seg {
        Segment::Text(t) => substitute_vars(t, vars),
        Segment::Styled { content, style_str } => StyleSpec::parse(style_str)
            .to_ansi_style()
            .paint(eval_segments(&parse_segments(content), vars))
            .to_string(),
        Segment::Conditional(inner) => eval_conditional(inner, vars),
    }
}

/// `(...)` ブロックの評価: 内包する変数がすべて空（またはマップにない）場合は非表示。
fn eval_conditional(inner: &[Segment], vars: &[(&str, &str)]) -> String {
    let refs = collect_var_refs(inner);
    if refs.is_empty() {
        return String::new();
    }
    let all_empty = refs.iter().all(|name| {
        vars.iter()
            .find(|(k, _)| k == name)
            .is_none_or(|(_, v)| v.is_empty())
    });
    if all_empty {
        String::new()
    } else {
        eval_segments(inner, vars)
    }
}

/// Segment ツリーから `$varname` 参照を収集する。
fn collect_var_refs(segs: &[Segment]) -> Vec<&str> {
    let mut refs = Vec::new();
    for seg in segs {
        let text = match seg {
            Segment::Text(t) => t.as_str(),
            Segment::Styled { content, .. } => content.as_str(),
            Segment::Conditional(inner) => {
                refs.extend(collect_var_refs(inner));
                continue;
            }
        };
        extract_var_names(text, &mut refs);
    }
    refs
}

fn extract_var_names<'a>(s: &'a str, out: &mut Vec<&'a str>) {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'$' {
            let start = i + 1;
            let end = bytes[start..]
                .iter()
                .position(|&b| !b.is_ascii_alphanumeric() && b != b'_')
                .map_or(bytes.len(), |pos| start + pos);
            if end > start {
                out.push(&s[start..end]);
            }
            i = end;
        } else {
            i += 1;
        }
    }
}

/// `$varname` を vars で置換する。マップにない変数はリテラルのまま残す。
fn substitute_vars(s: &str, vars: &[(&str, &str)]) -> String {
    let bytes = s.as_bytes();
    let mut result = String::with_capacity(s.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'$' {
            let start = i + 1;
            let end = bytes[start..]
                .iter()
                .position(|&b| !b.is_ascii_alphanumeric() && b != b'_')
                .map_or(bytes.len(), |pos| start + pos);
            if end > start {
                let name = &s[start..end];
                if let Some((_, val)) = vars.iter().find(|(k, _)| *k == name) {
                    result.push_str(val);
                } else {
                    result.push('$');
                    result.push_str(name);
                }
                i = end;
            } else {
                result.push('$');
                i += 1;
            }
        } else {
            result.push(bytes[i] as char);
            i += 1;
        }
    }
    result
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
        assert_eq!(render_token(&tok, None), "✓ Ready");
    }

    #[test]
    fn render_token_styled_contains_ansi() {
        let tok = TokenConfig {
            symbol: "✓".to_owned(),
            label: "Ready".to_owned(),
            format: "[$symbol](bold green) $label".to_owned(),
        };
        let out = render_token(&tok, None);
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
        let out = render_token(&tok, None);
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
        let out = render_token(&tok, None);
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
        assert_eq!(render_token(&tok, None), expected);
    }

    // ── Conditional ブロックのテスト ──────────────────────────────────────

    #[test]
    fn render_token_conditional_shown_when_pr_id_nonempty() {
        let tok = TokenConfig {
            symbol: "✓".to_owned(),
            label: "Ready for merge".to_owned(),
            format: "$symbol $label( #$pr_id)".to_owned(),
        };
        assert_eq!(render_token(&tok, Some("200")), "✓ Ready for merge #200");
    }

    #[test]
    fn render_token_conditional_hidden_when_pr_id_empty() {
        let tok = TokenConfig {
            symbol: "✓".to_owned(),
            label: "Ready for merge".to_owned(),
            format: "$symbol $label( #$pr_id)".to_owned(),
        };
        assert_eq!(render_token(&tok, Some("")), "✓ Ready for merge");
    }

    #[test]
    fn render_token_conditional_hidden_when_pr_id_none() {
        let tok = TokenConfig {
            symbol: "✓".to_owned(),
            label: "Ready for merge".to_owned(),
            format: "$symbol $label( #$pr_id)".to_owned(),
        };
        assert_eq!(render_token(&tok, None), "✓ Ready for merge");
    }

    #[test]
    fn render_token_conditional_no_vars_always_hidden() {
        let tok = TokenConfig {
            symbol: "✓".to_owned(),
            label: "Ready".to_owned(),
            format: "$symbol $label( static text)".to_owned(),
        };
        assert_eq!(render_token(&tok, Some("200")), "✓ Ready");
    }

    // ── $pr_id 置換のテスト ────────────────────────────────────────────────

    #[test]
    fn render_token_pr_id_substituted_when_some() {
        let tok = TokenConfig {
            symbol: "✓".to_owned(),
            label: "Ready for merge".to_owned(),
            format: "$symbol $label #$pr_id".to_owned(),
        };
        assert_eq!(render_token(&tok, Some("200")), "✓ Ready for merge #200");
    }

    #[test]
    fn render_token_pr_id_empty_string_leaves_hash() {
        let tok = TokenConfig {
            symbol: "✓".to_owned(),
            label: "Ready for merge".to_owned(),
            format: "$symbol $label #$pr_id".to_owned(),
        };
        assert_eq!(render_token(&tok, Some("")), "✓ Ready for merge #");
    }

    #[test]
    fn render_token_pr_id_not_substituted_when_none() {
        let tok = TokenConfig {
            symbol: "+".to_owned(),
            label: "Create PR".to_owned(),
            format: "$symbol $label".to_owned(),
        };
        assert_eq!(render_token(&tok, None), "+ Create PR");
    }

    #[test]
    fn render_token_pr_id_literal_remains_when_none() {
        let tok = TokenConfig {
            symbol: "+".to_owned(),
            label: "Create PR".to_owned(),
            format: "$symbol $label $pr_id".to_owned(),
        };
        // None を渡すと $pr_id は literal のまま残る
        assert_eq!(render_token(&tok, None), "+ Create PR $pr_id");
    }
}
