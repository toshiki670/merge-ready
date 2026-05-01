#[derive(Debug, PartialEq)]
pub(crate) enum Segment {
    Text(String),
    Styled { content: String, style_str: String },
}

pub(crate) fn parse_segments(format: &str) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut text_buf = String::new();
    let chars: Vec<char> = format.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '[' {
            // `]` を探す
            if let Some(close) = chars[i + 1..].iter().position(|&c| c == ']') {
                let close = close + i + 1;
                // `]` の直後が `(` か確認
                if close + 1 < chars.len() && chars[close + 1] == '(' {
                    // `)` を探す
                    if let Some(paren_close) = chars[close + 2..].iter().position(|&c| c == ')') {
                        let paren_close = paren_close + close + 2;
                        if !text_buf.is_empty() {
                            segments.push(Segment::Text(text_buf.clone()));
                            text_buf.clear();
                        }
                        let content: String = chars[i + 1..close].iter().collect();
                        let style_str: String = chars[close + 2..paren_close].iter().collect();
                        segments.push(Segment::Styled { content, style_str });
                        i = paren_close + 1;
                        continue;
                    }
                }
            }
        }
        text_buf.push(chars[i]);
        i += 1;
    }

    if !text_buf.is_empty() {
        segments.push(Segment::Text(text_buf));
    }

    segments
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_returns_single_text_segment() {
        let segs = parse_segments("$symbol $label");
        assert_eq!(segs, vec![Segment::Text("$symbol $label".to_owned())]);
    }

    #[test]
    fn styled_segment_is_parsed() {
        let segs = parse_segments("[$symbol](bold green) $label");
        assert_eq!(
            segs,
            vec![
                Segment::Styled {
                    content: "$symbol".to_owned(),
                    style_str: "bold green".to_owned(),
                },
                Segment::Text(" $label".to_owned()),
            ]
        );
    }

    #[test]
    fn unclosed_bracket_treated_as_text() {
        let segs = parse_segments("[$symbol] $label");
        assert_eq!(segs, vec![Segment::Text("[$symbol] $label".to_owned())]);
    }

    #[test]
    fn empty_style_is_styled_with_empty_str() {
        let segs = parse_segments("[$symbol]()");
        assert_eq!(
            segs,
            vec![Segment::Styled {
                content: "$symbol".to_owned(),
                style_str: String::new(),
            }]
        );
    }

    #[test]
    fn multiple_styled_segments() {
        let segs = parse_segments("[$symbol](bold red) [$label](green)");
        assert_eq!(
            segs,
            vec![
                Segment::Styled {
                    content: "$symbol".to_owned(),
                    style_str: "bold red".to_owned(),
                },
                Segment::Text(" ".to_owned()),
                Segment::Styled {
                    content: "$label".to_owned(),
                    style_str: "green".to_owned(),
                },
            ]
        );
    }

    #[test]
    fn empty_string_returns_empty_vec() {
        let segs = parse_segments("");
        assert_eq!(segs, vec![]);
    }

    #[test]
    fn text_before_styled_segment() {
        let segs = parse_segments("prefix [$symbol](red)");
        assert_eq!(
            segs,
            vec![
                Segment::Text("prefix ".to_owned()),
                Segment::Styled {
                    content: "$symbol".to_owned(),
                    style_str: "red".to_owned(),
                },
            ]
        );
    }
}
