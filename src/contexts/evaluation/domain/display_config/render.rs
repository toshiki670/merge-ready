use super::{CompiledErrorConfig, CompiledTokenConfig};
use crate::contexts::evaluation::domain::format_parser::CompiledSegment;

/// `pr_ids` が `Some` の場合は `$pr_ids` を置換する。`None` の場合は `$pr_ids` を literal のまま残す。
/// 値は `#1734 #2669` のように `#` 付き・スペース区切り（複数 PR 時）か、空文字（単一 PR 時）。
/// `(...)` ブロック内の変数がすべて空の場合、そのブロックは出力されない。
///
/// `token` は前計算済みの [`CompiledTokenConfig`]。`format` パースと `StyleSpec::parse`
/// はロード時に済んでいるので、ここでは変数置換とツリー評価だけを行う。
#[must_use]
pub fn render_token(token: &CompiledTokenConfig, pr_ids: Option<&str>) -> String {
    let mut vars: Vec<(&str, &str)> = vec![("symbol", &token.symbol), ("label", &token.label)];
    if let Some(ids) = pr_ids {
        vars.push(("pr_ids", ids));
    }
    eval_segments(&token.segments, &vars)
}

#[must_use]
pub fn render_error_token(config: &CompiledErrorConfig, message: &str) -> String {
    let vars: Vec<(&str, &str)> = vec![("symbol", &config.symbol), ("message", message)];
    eval_segments(&config.segments, &vars)
}

fn eval_segments(segs: &[CompiledSegment], vars: &[(&str, &str)]) -> String {
    segs.iter().map(|s| eval_segment(s, vars)).collect()
}

fn eval_segment(seg: &CompiledSegment, vars: &[(&str, &str)]) -> String {
    match seg {
        CompiledSegment::Text(t) => substitute_vars(t, vars),
        CompiledSegment::Styled { content, style } => style
            .to_ansi_style()
            .paint(eval_segments(content, vars))
            .to_string(),
        CompiledSegment::Conditional(inner) => eval_conditional(inner, vars),
    }
}

/// `(...)` ブロックの評価: 内包する変数がすべて空（またはマップにない）場合は非表示。
fn eval_conditional(inner: &[CompiledSegment], vars: &[(&str, &str)]) -> String {
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

/// `CompiledSegment` ツリーから `$varname` 参照を収集する。
fn collect_var_refs(segs: &[CompiledSegment]) -> Vec<&str> {
    let mut refs = Vec::new();
    for seg in segs {
        match seg {
            CompiledSegment::Text(t) => extract_var_names(t, &mut refs),
            CompiledSegment::Styled { content, .. } => refs.extend(collect_var_refs(content)),
            CompiledSegment::Conditional(inner) => refs.extend(collect_var_refs(inner)),
        }
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
            // 次の `$` までの非変数部分をまとめてコピーする。`$` は ASCII (1 バイト) なので
            // i は常に char 境界に揃い、マルチバイト文字も保持される。
            let next = bytes[i..]
                .iter()
                .position(|&b| b == b'$')
                .map_or(bytes.len(), |pos| i + pos);
            result.push_str(&s[i..next]);
            i = next;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use crate::contexts::evaluation::domain::display_config::{ErrorConfig, TokenConfig};

    // テストは `TokenConfig`/`ErrorConfig` を一度 compile してから render する。
    // 期待する出力契約は前計算化の前後で不変。
    fn render_token(token: &TokenConfig, pr_ids: Option<&str>) -> String {
        super::render_token(&token.compile(), pr_ids)
    }

    fn render_error_token(config: &ErrorConfig, message: &str) -> String {
        super::render_error_token(&config.compile(), message)
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
        assert!(reset_pos < label_pos);
        let after_reset = &out[reset_pos + reset.len()..];
        assert!(!after_reset.contains("\x1b["));
    }

    #[test]
    fn render_token_plain_format_identical_to_simple_replace() {
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

    #[test]
    fn render_token_conditional_shown_when_pr_ids_nonempty() {
        let tok = TokenConfig {
            symbol: "✓".to_owned(),
            label: "Ready for merge".to_owned(),
            format: "$symbol $label( $pr_ids)".to_owned(),
        };
        assert_eq!(
            render_token(&tok, Some("#200 #201")),
            "✓ Ready for merge #200 #201"
        );
    }

    #[test]
    fn render_token_conditional_hidden_when_pr_ids_empty() {
        let tok = TokenConfig {
            symbol: "✓".to_owned(),
            label: "Ready for merge".to_owned(),
            format: "$symbol $label( $pr_ids)".to_owned(),
        };
        assert_eq!(render_token(&tok, Some("")), "✓ Ready for merge");
    }

    #[test]
    fn render_token_conditional_hidden_when_pr_ids_none() {
        let tok = TokenConfig {
            symbol: "✓".to_owned(),
            label: "Ready for merge".to_owned(),
            format: "$symbol $label( $pr_ids)".to_owned(),
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

    #[test]
    fn render_token_pr_ids_substituted_when_some() {
        let tok = TokenConfig {
            symbol: "✓".to_owned(),
            label: "Ready for merge".to_owned(),
            format: "$symbol $label $pr_ids".to_owned(),
        };
        assert_eq!(
            render_token(&tok, Some("#200 #201")),
            "✓ Ready for merge #200 #201"
        );
    }

    #[test]
    fn render_token_pr_ids_empty_string_substitutes_empty() {
        let tok = TokenConfig {
            symbol: "✓".to_owned(),
            label: "Ready for merge".to_owned(),
            format: "$symbol $label$pr_ids".to_owned(),
        };
        assert_eq!(render_token(&tok, Some("")), "✓ Ready for merge");
    }

    #[test]
    fn render_token_pr_ids_not_substituted_when_none() {
        let tok = TokenConfig {
            symbol: "+".to_owned(),
            label: "Create PR".to_owned(),
            format: "$symbol $label".to_owned(),
        };
        assert_eq!(render_token(&tok, None), "+ Create PR");
    }

    #[test]
    fn render_token_pr_ids_literal_remains_when_none() {
        let tok = TokenConfig {
            symbol: "+".to_owned(),
            label: "Create PR".to_owned(),
            format: "$symbol $label $pr_ids".to_owned(),
        };
        assert_eq!(render_token(&tok, None), "+ Create PR $pr_ids");
    }

    #[test]
    fn unknown_variable_remains_literal() {
        let tok = TokenConfig {
            symbol: "+".to_owned(),
            label: "Create PR".to_owned(),
            format: "$symbol $unknown".to_owned(),
        };
        assert_eq!(render_token(&tok, None), "+ $unknown");
    }

    #[test]
    fn render_token_non_ascii_literal_in_format_preserved() {
        let tok = TokenConfig {
            symbol: "✓".to_owned(),
            label: "Ready for merge".to_owned(),
            format: "【$symbol】準備完了: $label 🎉".to_owned(),
        };
        assert_eq!(
            render_token(&tok, None),
            "【✓】準備完了: Ready for merge 🎉"
        );
    }
}
