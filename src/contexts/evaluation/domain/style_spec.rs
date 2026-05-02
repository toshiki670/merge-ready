use nu_ansi_term::{Color, Style};

#[derive(Debug, PartialEq)]
pub(crate) enum NamedColor {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Purple,
    Cyan,
    White,
    BrightBlack,
    BrightRed,
    BrightGreen,
    BrightYellow,
    BrightBlue,
    BrightPurple,
    BrightCyan,
    BrightWhite,
}

#[derive(Debug, PartialEq)]
pub(crate) enum ColorSpec {
    Named(NamedColor),
    Ansi256(u8),
    Rgb(u8, u8, u8),
}

#[derive(Debug, PartialEq, Default)]
#[allow(clippy::struct_excessive_bools)]
pub(crate) struct StyleSpec {
    pub fg: Option<ColorSpec>,
    pub bg: Option<ColorSpec>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub dimmed: bool,
    pub inverted: bool,
    pub blink: bool,
    pub hidden: bool,
    pub strikethrough: bool,
    pub none: bool,
}

impl StyleSpec {
    pub(crate) fn parse(s: &str) -> Self {
        let mut spec = Self::default();
        for token in s.split_whitespace() {
            let lower = token.to_ascii_lowercase();
            match lower.as_str() {
                "bold" => spec.bold = true,
                "italic" => spec.italic = true,
                "underline" => spec.underline = true,
                "dimmed" => spec.dimmed = true,
                "inverted" => spec.inverted = true,
                "blink" => spec.blink = true,
                "hidden" => spec.hidden = true,
                "strikethrough" => spec.strikethrough = true,
                "none" => spec.none = true,
                name if parse_named_color(name).is_some() => {
                    spec.fg = Some(ColorSpec::Named(parse_named_color(name).unwrap()));
                }
                s if s.starts_with("fg:") => {
                    if let Some(color) = parse_color_value(&s["fg:".len()..]) {
                        spec.fg = Some(color);
                    }
                }
                s if s.starts_with("bg:") => {
                    if let Some(color) = parse_color_value(&s["bg:".len()..]) {
                        spec.bg = Some(color);
                    }
                }
                _ => {}
            }
        }
        spec
    }

    pub(crate) fn to_ansi_style(&self) -> Style {
        if self.none {
            return Style::new();
        }
        let mut style = Style::new();
        if let Some(fg) = &self.fg {
            style = style.fg(color_spec_to_nu(fg));
        }
        if let Some(bg) = &self.bg {
            style = style.on(color_spec_to_nu(bg));
        }
        if self.bold {
            style = style.bold();
        }
        if self.italic {
            style = style.italic();
        }
        if self.underline {
            style = style.underline();
        }
        if self.dimmed {
            style = style.dimmed();
        }
        if self.inverted {
            style = style.reverse();
        }
        if self.blink {
            style = style.blink();
        }
        if self.hidden {
            style = style.hidden();
        }
        if self.strikethrough {
            style = style.strikethrough();
        }
        style
    }
}

fn parse_named_color(s: &str) -> Option<NamedColor> {
    match s {
        "black" => Some(NamedColor::Black),
        "red" => Some(NamedColor::Red),
        "green" => Some(NamedColor::Green),
        "yellow" => Some(NamedColor::Yellow),
        "blue" => Some(NamedColor::Blue),
        "purple" => Some(NamedColor::Purple),
        "cyan" => Some(NamedColor::Cyan),
        "white" => Some(NamedColor::White),
        "bright-black" => Some(NamedColor::BrightBlack),
        "bright-red" => Some(NamedColor::BrightRed),
        "bright-green" => Some(NamedColor::BrightGreen),
        "bright-yellow" => Some(NamedColor::BrightYellow),
        "bright-blue" => Some(NamedColor::BrightBlue),
        "bright-purple" => Some(NamedColor::BrightPurple),
        "bright-cyan" => Some(NamedColor::BrightCyan),
        "bright-white" => Some(NamedColor::BrightWhite),
        _ => None,
    }
}

fn parse_color_value(s: &str) -> Option<ColorSpec> {
    if let Some(hex) = s.strip_prefix('#').filter(|h| h.len() == 6) {
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        return Some(ColorSpec::Rgb(r, g, b));
    }
    if let Ok(n) = s.parse::<u8>() {
        return Some(ColorSpec::Ansi256(n));
    }
    parse_named_color(s).map(ColorSpec::Named)
}

fn named_color_to_nu(c: &NamedColor) -> Color {
    match c {
        NamedColor::Black => Color::Black,
        NamedColor::Red => Color::Red,
        NamedColor::Green => Color::Green,
        NamedColor::Yellow => Color::Yellow,
        NamedColor::Blue => Color::Blue,
        NamedColor::Purple => Color::Purple,
        NamedColor::Cyan => Color::Cyan,
        NamedColor::White => Color::White,
        NamedColor::BrightBlack => Color::DarkGray,
        NamedColor::BrightRed => Color::LightRed,
        NamedColor::BrightGreen => Color::LightGreen,
        NamedColor::BrightYellow => Color::LightYellow,
        NamedColor::BrightBlue => Color::LightBlue,
        NamedColor::BrightPurple => Color::LightPurple,
        NamedColor::BrightCyan => Color::LightCyan,
        NamedColor::BrightWhite => Color::LightGray,
    }
}

fn color_spec_to_nu(spec: &ColorSpec) -> Color {
    match spec {
        ColorSpec::Named(n) => named_color_to_nu(n),
        ColorSpec::Ansi256(n) => Color::Fixed(*n),
        ColorSpec::Rgb(r, g, b) => Color::Rgb(*r, *g, *b),
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    // ── fg カラー解析 ────────────────────────────────────────────────────────

    #[rstest]
    #[case("green", Some(ColorSpec::Named(NamedColor::Green)))]
    #[case("GREEN", Some(ColorSpec::Named(NamedColor::Green)))]
    #[case("bright-cyan", Some(ColorSpec::Named(NamedColor::BrightCyan)))]
    #[case("fg:blue", Some(ColorSpec::Named(NamedColor::Blue)))]
    #[case("fg:196", Some(ColorSpec::Ansi256(196)))]
    #[case("fg:#ff8800", Some(ColorSpec::Rgb(0xff, 0x88, 0x00)))]
    fn fg_color_cases(#[case] input: &str, #[case] expected: Option<ColorSpec>) {
        assert_eq!(StyleSpec::parse(input).fg, expected);
    }

    // ── bg カラー解析 ────────────────────────────────────────────────────────

    #[rstest]
    #[case("bg:red", Some(ColorSpec::Named(NamedColor::Red)))]
    #[case("bg:208", Some(ColorSpec::Ansi256(208)))]
    #[case("bg:#001122", Some(ColorSpec::Rgb(0x00, 0x11, 0x22)))]
    fn bg_color_cases(#[case] input: &str, #[case] expected: Option<ColorSpec>) {
        assert_eq!(StyleSpec::parse(input).bg, expected);
    }

    // ── 不正・未知指定子が有効な色を上書きしない ─────────────────────────────

    #[rstest]
    #[case("bold xyzzy green", Some(ColorSpec::Named(NamedColor::Green)))]
    #[case("green fg:typo", Some(ColorSpec::Named(NamedColor::Green)))]
    fn invalid_token_does_not_clear_fg(#[case] input: &str, #[case] expected: Option<ColorSpec>) {
        assert_eq!(StyleSpec::parse(input).fg, expected);
    }

    #[rstest]
    #[case("bg:red bg:typo", Some(ColorSpec::Named(NamedColor::Red)))]
    fn invalid_token_does_not_clear_bg(#[case] input: &str, #[case] expected: Option<ColorSpec>) {
        assert_eq!(StyleSpec::parse(input).bg, expected);
    }

    // ── その他 ───────────────────────────────────────────────────────────────

    #[test]
    fn parse_bold_green() {
        let s = StyleSpec::parse("bold green");
        assert!(s.bold);
        assert_eq!(s.fg, Some(ColorSpec::Named(NamedColor::Green)));
    }

    #[test]
    fn parse_none() {
        let s = StyleSpec::parse("none");
        assert!(s.none);
        assert!(!s.bold);
        assert!(s.fg.is_none());
    }

    #[test]
    fn parse_all_attributes() {
        let s = StyleSpec::parse("italic underline dimmed inverted blink hidden strikethrough");
        assert!(s.italic);
        assert!(s.underline);
        assert!(s.dimmed);
        assert!(s.inverted);
        assert!(s.blink);
        assert!(s.hidden);
        assert!(s.strikethrough);
    }

    #[test]
    fn to_ansi_style_bold_green_has_effects_and_color() {
        let s = StyleSpec::parse("bold green");
        let style = s.to_ansi_style();
        assert!(style.is_bold);
        assert_eq!(style.foreground, Some(Color::Green));
    }

    #[test]
    fn to_ansi_style_none_returns_empty_style() {
        let s = StyleSpec::parse("none");
        let style = s.to_ansi_style();
        assert_eq!(style, Style::new());
    }
}
