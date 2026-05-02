#[derive(Debug, PartialEq)]
pub(crate) enum Segment {
    Text(String),
    Styled { content: String, style_str: String },
}

/// `[text](style)` 構文を `Segment` 列に分解する。
///
/// `]` の直後が `(` でない場合は Styled として扱わず Text に含める（後方互換）。
/// ASCII の `[` `]` `(` `)` はすべて 1 バイトなので `str::find` でバイト操作しても安全。
pub(crate) fn parse_segments(format: &str) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut remaining = format;
    let mut text_acc = String::new();

    while !remaining.is_empty() {
        if let Some(open) = remaining.find('[') {
            let after_open = &remaining[open + 1..];

            if let Some(close_bracket) = after_open.find(']') {
                let after_bracket = &after_open[close_bracket + 1..];

                if let Some(after_paren) = after_bracket.strip_prefix('(')
                    && let Some(paren_close) = after_paren.find(')')
                {
                    text_acc.push_str(&remaining[..open]);
                    if !text_acc.is_empty() {
                        segments.push(Segment::Text(std::mem::take(&mut text_acc)));
                    }
                    segments.push(Segment::Styled {
                        content: after_open[..close_bracket].to_owned(),
                        style_str: after_paren[..paren_close].to_owned(),
                    });
                    remaining = &after_paren[paren_close + 1..];
                    continue;
                }
            }

            // `[` はスタイル構文の開始ではない — テキストとして蓄積
            text_acc.push_str(&remaining[..=open]);
            remaining = &remaining[open + 1..];
        } else {
            text_acc.push_str(remaining);
            remaining = "";
        }
    }

    if !text_acc.is_empty() {
        segments.push(Segment::Text(text_acc));
    }

    segments
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    // ── プレーンテキスト（Styled 構文なし）───────────────────────────────────

    #[rstest]
    #[case("$symbol $label", vec![Segment::Text("$symbol $label".to_owned())])]
    #[case("[$symbol] $label", vec![Segment::Text("[$symbol] $label".to_owned())])]
    #[case("", vec![])]
    fn plain_text_cases(#[case] input: &str, #[case] expected: Vec<Segment>) {
        assert_eq!(parse_segments(input), expected);
    }

    // ── Styled セグメントのパース ────────────────────────────────────────────

    #[rstest]
    #[case(
        "[$symbol](bold green) $label",
        vec![
            Segment::Styled { content: "$symbol".to_owned(), style_str: "bold green".to_owned() },
            Segment::Text(" $label".to_owned()),
        ]
    )]
    #[case(
        "[$symbol]()",
        vec![Segment::Styled { content: "$symbol".to_owned(), style_str: String::new() }]
    )]
    #[case(
        "prefix [$symbol](red)",
        vec![
            Segment::Text("prefix ".to_owned()),
            Segment::Styled { content: "$symbol".to_owned(), style_str: "red".to_owned() },
        ]
    )]
    #[case(
        "[$symbol](bold red) [$label](green)",
        vec![
            Segment::Styled { content: "$symbol".to_owned(), style_str: "bold red".to_owned() },
            Segment::Text(" ".to_owned()),
            Segment::Styled { content: "$label".to_owned(), style_str: "green".to_owned() },
        ]
    )]
    fn styled_segment_cases(#[case] input: &str, #[case] expected: Vec<Segment>) {
        assert_eq!(parse_segments(input), expected);
    }
}
