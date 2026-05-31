use crate::contexts::evaluation::domain::style_spec::StyleSpec;

enum Either<L, R> {
    Left(L),
    Right(R),
}

#[derive(Debug, PartialEq)]
pub(crate) enum Segment {
    Text(String),
    Styled { content: String, style_str: String },
    Conditional(Vec<Segment>),
}

/// `Segment` をロード時に前計算した render 用の値オブジェクト。
///
/// `Styled` は `style_str` を解決済み [`StyleSpec`] として、`content` を再帰
/// compile 済みの `Vec<CompiledSegment>` として保持する。これにより render 時は
/// `parse_segments` も `StyleSpec::parse` も呼ばずツリーを評価するだけで済む。
#[derive(Debug, PartialEq)]
pub(crate) enum CompiledSegment {
    Text(String),
    Styled {
        content: Vec<CompiledSegment>,
        style: StyleSpec,
    },
    Conditional(Vec<CompiledSegment>),
}

/// `format` 文字列を一度だけパースし、各 `Segment` を前計算済みの
/// [`CompiledSegment`] へ変換する。設定ロード時に一度だけ呼ぶ。
pub(crate) fn compile_segments(format: &str) -> Vec<CompiledSegment> {
    parse_segments(format)
        .into_iter()
        .map(compile_segment)
        .collect()
}

fn compile_segment(segment: Segment) -> CompiledSegment {
    match segment {
        Segment::Text(t) => CompiledSegment::Text(t),
        Segment::Styled { content, style_str } => CompiledSegment::Styled {
            content: compile_segments(&content),
            style: StyleSpec::parse(&style_str),
        },
        Segment::Conditional(inner) => {
            CompiledSegment::Conditional(inner.into_iter().map(compile_segment).collect())
        }
    }
}

/// `[text](style)` および `(content)` 構文を `Segment` 列に分解する。
///
/// - `[text](style)` → `Segment::Styled`
/// - `(content)` → `Segment::Conditional`（内側を再帰パース）
/// - `]` の直後が `(` でない場合は Styled として扱わず Text に含める（後方互換）。
/// - ASCII の `[` `]` `(` `)` はすべて 1 バイトなので `str::find` でバイト操作しても安全。
pub(crate) fn parse_segments(format: &str) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut remaining = format;
    let mut text_acc = String::new();

    while !remaining.is_empty() {
        let bracket_pos = remaining.find('[');
        let paren_pos = remaining.find('(');

        // 先に現れるデリミタを判定する
        let next = match (bracket_pos, paren_pos) {
            (None, None) => None,
            (Some(b), None) => Some(Either::Left(b)),
            (None, Some(p)) => Some(Either::Right(p)),
            (Some(b), Some(p)) => Some(if b <= p {
                Either::Left(b)
            } else {
                Either::Right(p)
            }),
        };

        match next {
            // デリミタなし → 残りをすべて Text に
            None => {
                text_acc.push_str(remaining);
                remaining = "";
            }
            // `[` が先 → `[text](style)` を試みる
            Some(Either::Left(b)) => {
                let after_open = &remaining[b + 1..];
                if let Some(close_bracket) = after_open.find(']') {
                    let after_bracket = &after_open[close_bracket + 1..];
                    if let Some(after_paren) = after_bracket.strip_prefix('(')
                        && let Some(paren_close) = after_paren.find(')')
                    {
                        text_acc.push_str(&remaining[..b]);
                        flush_text_acc(&mut text_acc, &mut segments);
                        segments.push(Segment::Styled {
                            content: after_open[..close_bracket].to_owned(),
                            style_str: after_paren[..paren_close].to_owned(),
                        });
                        remaining = &after_paren[paren_close + 1..];
                        continue;
                    }
                }
                // Styled 構文にならなかった — `[` を Text に蓄積
                text_acc.push_str(&remaining[..=b]);
                remaining = &remaining[b + 1..];
            }
            // `(` が先 → Conditional を試みる
            Some(Either::Right(p)) => {
                let after_paren = &remaining[p + 1..];
                if let Some(close) = after_paren.find(')') {
                    text_acc.push_str(&remaining[..p]);
                    flush_text_acc(&mut text_acc, &mut segments);
                    let inner = parse_segments(&after_paren[..close]);
                    segments.push(Segment::Conditional(inner));
                    remaining = &after_paren[close + 1..];
                    continue;
                }
                // `)` がない — `(` を Text に蓄積
                text_acc.push_str(&remaining[..=p]);
                remaining = &remaining[p + 1..];
            }
        }
    }

    if !text_acc.is_empty() {
        segments.push(Segment::Text(text_acc));
    }

    segments
}

fn flush_text_acc(text_acc: &mut String, segments: &mut Vec<Segment>) {
    if !text_acc.is_empty() {
        segments.push(Segment::Text(std::mem::take(text_acc)));
    }
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

    // ── Conditional セグメントのパース ──────────────────────────────────────

    #[rstest]
    #[case(
        "( $pr_ids)",
        vec![Segment::Conditional(vec![Segment::Text(" $pr_ids".to_owned())])]
    )]
    #[case(
        "$symbol( $pr_ids)",
        vec![
            Segment::Text("$symbol".to_owned()),
            Segment::Conditional(vec![Segment::Text(" $pr_ids".to_owned())]),
        ]
    )]
    #[case(
        "()",
        vec![Segment::Conditional(vec![])]
    )]
    #[case(
        "(no close",
        vec![Segment::Text("(no close".to_owned())]
    )]
    fn conditional_segment_cases(#[case] input: &str, #[case] expected: Vec<Segment>) {
        assert_eq!(parse_segments(input), expected);
    }

    #[test]
    fn styled_then_conditional() {
        assert_eq!(
            parse_segments("[$s](red)( suffix)"),
            vec![
                Segment::Styled {
                    content: "$s".to_owned(),
                    style_str: "red".to_owned()
                },
                Segment::Conditional(vec![Segment::Text(" suffix".to_owned())]),
            ]
        );
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

    // ── compile_segments: parse_segments + StyleSpec::parse の前計算 ─────────────

    #[test]
    fn compile_resolves_style_and_nests_content() {
        use crate::contexts::evaluation::domain::style_spec::StyleSpec;

        assert_eq!(
            compile_segments("[$symbol](bold green) $label"),
            vec![
                CompiledSegment::Styled {
                    content: vec![CompiledSegment::Text("$symbol".to_owned())],
                    style: StyleSpec::parse("bold green"),
                },
                CompiledSegment::Text(" $label".to_owned()),
            ]
        );
    }

    #[test]
    fn compile_preserves_conditional_and_plain_text() {
        assert_eq!(
            compile_segments("$symbol $label( $pr_ids)"),
            vec![
                CompiledSegment::Text("$symbol $label".to_owned()),
                CompiledSegment::Conditional(vec![CompiledSegment::Text(" $pr_ids".to_owned())]),
            ]
        );
    }
}
