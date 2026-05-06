mod color;
mod modifiers;

use color::{ColorSpec, color_spec_to_nu, parse_color_value, parse_named_color};
use modifiers::StyleModifiers;
use nu_ansi_term::Style;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum StyleSpec {
    None,
    Styled {
        fg: Option<ColorSpec>,
        bg: Option<ColorSpec>,
        modifiers: StyleModifiers,
    },
}

impl Default for StyleSpec {
    fn default() -> Self {
        Self::Styled {
            fg: None,
            bg: None,
            modifiers: StyleModifiers::empty(),
        }
    }
}

impl StyleSpec {
    pub(crate) fn parse(s: &str) -> Self {
        let mut fg = None;
        let mut bg = None;
        let mut modifiers = StyleModifiers::empty();

        for token in s.split_whitespace() {
            let lower = token.to_ascii_lowercase();

            if lower == "none" {
                return Self::None;
            }
            if let Some(modifier) = StyleModifiers::parse_token(&lower) {
                modifiers.insert(modifier);
                continue;
            }
            if let Some(color) = parse_named_color(&lower) {
                fg = Some(ColorSpec::Named(color));
                continue;
            }
            if let Some(value) = lower.strip_prefix("fg:") {
                if let Some(color) = parse_color_value(value) {
                    fg = Some(color);
                }
                continue;
            }
            if let Some(value) = lower.strip_prefix("bg:")
                && let Some(color) = parse_color_value(value)
            {
                bg = Some(color);
            }
        }

        Self::Styled { fg, bg, modifiers }
    }

    pub(crate) fn to_ansi_style(&self) -> Style {
        match self {
            Self::None => Style::new(),
            Self::Styled { fg, bg, modifiers } => {
                let mut style = Style::new();
                if let Some(fg) = fg {
                    style = style.fg(color_spec_to_nu(fg));
                }
                if let Some(bg) = bg {
                    style = style.on(color_spec_to_nu(bg));
                }
                (*modifiers).apply_to(style)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use color::{ColorSpec, NamedColor};
    use modifiers::StyleModifiers;
    use nu_ansi_term::Color;
    use rstest::rstest;

    use super::*;

    fn parse_styled(input: &str) -> (Option<ColorSpec>, Option<ColorSpec>, StyleModifiers) {
        match StyleSpec::parse(input) {
            StyleSpec::Styled { fg, bg, modifiers } => (fg, bg, modifiers),
            StyleSpec::None => panic!("expected styled spec"),
        }
    }

    #[rstest]
    #[case("green", Some(ColorSpec::Named(NamedColor::Green)))]
    #[case("GREEN", Some(ColorSpec::Named(NamedColor::Green)))]
    #[case("bright-cyan", Some(ColorSpec::Named(NamedColor::BrightCyan)))]
    #[case("fg:blue", Some(ColorSpec::Named(NamedColor::Blue)))]
    #[case("fg:196", Some(ColorSpec::Ansi256(196)))]
    #[case("fg:#ff8800", Some(ColorSpec::Rgb(0xff, 0x88, 0x00)))]
    fn fg_color_cases(#[case] input: &str, #[case] expected: Option<ColorSpec>) {
        let (fg, _, _) = parse_styled(input);

        assert_eq!(fg, expected);
    }

    #[rstest]
    #[case("bg:red", Some(ColorSpec::Named(NamedColor::Red)))]
    #[case("bg:208", Some(ColorSpec::Ansi256(208)))]
    #[case("bg:#001122", Some(ColorSpec::Rgb(0x00, 0x11, 0x22)))]
    fn bg_color_cases(#[case] input: &str, #[case] expected: Option<ColorSpec>) {
        let (_, bg, _) = parse_styled(input);

        assert_eq!(bg, expected);
    }

    #[rstest]
    #[case("bold xyzzy green", Some(ColorSpec::Named(NamedColor::Green)))]
    #[case("green fg:typo", Some(ColorSpec::Named(NamedColor::Green)))]
    fn invalid_token_does_not_clear_fg(#[case] input: &str, #[case] expected: Option<ColorSpec>) {
        let (fg, _, _) = parse_styled(input);

        assert_eq!(fg, expected);
    }

    #[rstest]
    #[case("bg:red bg:typo", Some(ColorSpec::Named(NamedColor::Red)))]
    fn invalid_token_does_not_clear_bg(#[case] input: &str, #[case] expected: Option<ColorSpec>) {
        let (_, bg, _) = parse_styled(input);

        assert_eq!(bg, expected);
    }

    #[test]
    fn parse_bold_green() {
        let (fg, _, modifiers) = parse_styled("bold green");

        assert!(modifiers.contains(StyleModifiers::BOLD));
        assert_eq!(fg, Some(ColorSpec::Named(NamedColor::Green)));
    }

    #[test]
    fn parse_none() {
        assert_eq!(StyleSpec::parse("none"), StyleSpec::None);
    }

    #[test]
    fn parse_all_attributes() {
        let (_, _, modifiers) =
            parse_styled("italic underline dimmed inverted blink hidden strikethrough");

        assert!(modifiers.contains(StyleModifiers::ITALIC));
        assert!(modifiers.contains(StyleModifiers::UNDERLINE));
        assert!(modifiers.contains(StyleModifiers::DIMMED));
        assert!(modifiers.contains(StyleModifiers::INVERTED));
        assert!(modifiers.contains(StyleModifiers::BLINK));
        assert!(modifiers.contains(StyleModifiers::HIDDEN));
        assert!(modifiers.contains(StyleModifiers::STRIKETHROUGH));
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

    #[test]
    fn to_ansi_style_none_with_other_tokens_returns_empty_style() {
        let s = StyleSpec::parse("bold green none");
        let style = s.to_ansi_style();

        assert_eq!(style, Style::new());
    }
}
