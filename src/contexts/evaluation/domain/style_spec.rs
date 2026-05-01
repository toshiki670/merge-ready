use anstyle::{AnsiColor, Color, Effects, RgbColor, Style};

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
                // 色名（前景色として扱う）
                name if parse_named_color(name).is_some() => {
                    spec.fg = Some(ColorSpec::Named(parse_named_color(name).unwrap()));
                }
                // fg:... / bg:...
                s if s.starts_with("fg:") => {
                    spec.fg = parse_color_value(&s["fg:".len()..]);
                }
                s if s.starts_with("bg:") => {
                    spec.bg = parse_color_value(&s["bg:".len()..]);
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
            style = style.fg_color(Some(color_spec_to_anstyle(fg)));
        }
        if let Some(bg) = &self.bg {
            style = style.bg_color(Some(color_spec_to_anstyle(bg)));
        }
        let mut effects = Effects::new();
        if self.bold {
            effects |= Effects::BOLD;
        }
        if self.italic {
            effects |= Effects::ITALIC;
        }
        if self.underline {
            effects |= Effects::UNDERLINE;
        }
        if self.dimmed {
            effects |= Effects::DIMMED;
        }
        if self.inverted {
            effects |= Effects::INVERT;
        }
        if self.blink {
            effects |= Effects::BLINK;
        }
        if self.hidden {
            effects |= Effects::HIDDEN;
        }
        if self.strikethrough {
            effects |= Effects::STRIKETHROUGH;
        }
        style.effects(effects)
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
    // #rrggbb
    if let Some(hex) = s.strip_prefix('#').filter(|h| h.len() == 6) {
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        return Some(ColorSpec::Rgb(r, g, b));
    }
    // 数値（0–255）
    if let Ok(n) = s.parse::<u8>() {
        return Some(ColorSpec::Ansi256(n));
    }
    // 色名
    parse_named_color(s).map(ColorSpec::Named)
}

fn named_color_to_ansi(c: &NamedColor) -> AnsiColor {
    match c {
        NamedColor::Black => AnsiColor::Black,
        NamedColor::Red => AnsiColor::Red,
        NamedColor::Green => AnsiColor::Green,
        NamedColor::Yellow => AnsiColor::Yellow,
        NamedColor::Blue => AnsiColor::Blue,
        NamedColor::Purple => AnsiColor::Magenta,
        NamedColor::Cyan => AnsiColor::Cyan,
        NamedColor::White => AnsiColor::White,
        NamedColor::BrightBlack => AnsiColor::BrightBlack,
        NamedColor::BrightRed => AnsiColor::BrightRed,
        NamedColor::BrightGreen => AnsiColor::BrightGreen,
        NamedColor::BrightYellow => AnsiColor::BrightYellow,
        NamedColor::BrightBlue => AnsiColor::BrightBlue,
        NamedColor::BrightPurple => AnsiColor::BrightMagenta,
        NamedColor::BrightCyan => AnsiColor::BrightCyan,
        NamedColor::BrightWhite => AnsiColor::BrightWhite,
    }
}

fn color_spec_to_anstyle(spec: &ColorSpec) -> Color {
    match spec {
        ColorSpec::Named(n) => Color::Ansi(named_color_to_ansi(n)),
        ColorSpec::Ansi256(n) => Color::Ansi256(anstyle::Ansi256Color(*n)),
        ColorSpec::Rgb(r, g, b) => Color::Rgb(RgbColor(*r, *g, *b)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_bold_green() {
        let s = StyleSpec::parse("bold green");
        assert!(s.bold);
        assert_eq!(s.fg, Some(ColorSpec::Named(NamedColor::Green)));
    }

    #[test]
    fn parse_fg_256() {
        let s = StyleSpec::parse("fg:196");
        assert_eq!(s.fg, Some(ColorSpec::Ansi256(196)));
    }

    #[test]
    fn parse_fg_hex() {
        let s = StyleSpec::parse("fg:#ff8800");
        assert_eq!(s.fg, Some(ColorSpec::Rgb(0xff, 0x88, 0x00)));
    }

    #[test]
    fn parse_bg_named() {
        let s = StyleSpec::parse("bg:red");
        assert_eq!(s.bg, Some(ColorSpec::Named(NamedColor::Red)));
    }

    #[test]
    fn parse_bright_modifier() {
        let s = StyleSpec::parse("bright-cyan");
        assert_eq!(s.fg, Some(ColorSpec::Named(NamedColor::BrightCyan)));
    }

    #[test]
    fn parse_none() {
        let s = StyleSpec::parse("none");
        assert!(s.none);
        assert!(!s.bold);
        assert!(s.fg.is_none());
    }

    #[test]
    fn parse_case_insensitive() {
        let s = StyleSpec::parse("BOLD GREEN");
        assert!(s.bold);
        assert_eq!(s.fg, Some(ColorSpec::Named(NamedColor::Green)));
    }

    #[test]
    fn unknown_token_is_ignored() {
        let s = StyleSpec::parse("bold xyzzy green");
        assert!(s.bold);
        assert_eq!(s.fg, Some(ColorSpec::Named(NamedColor::Green)));
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
    fn parse_fg_prefix_named() {
        let s = StyleSpec::parse("fg:blue");
        assert_eq!(s.fg, Some(ColorSpec::Named(NamedColor::Blue)));
    }

    #[test]
    fn parse_bg_256() {
        let s = StyleSpec::parse("bg:208");
        assert_eq!(s.bg, Some(ColorSpec::Ansi256(208)));
    }

    #[test]
    fn parse_bg_hex() {
        let s = StyleSpec::parse("bg:#001122");
        assert_eq!(s.bg, Some(ColorSpec::Rgb(0x00, 0x11, 0x22)));
    }

    #[test]
    fn to_ansi_style_bold_green_has_effects_and_color() {
        let s = StyleSpec::parse("bold green");
        let style = s.to_ansi_style();
        assert!(style.get_effects().contains(Effects::BOLD));
        assert_eq!(style.get_fg_color(), Some(Color::Ansi(AnsiColor::Green)));
    }

    #[test]
    fn to_ansi_style_none_returns_empty_style() {
        let s = StyleSpec::parse("none");
        let style = s.to_ansi_style();
        assert_eq!(style, Style::new());
    }
}
