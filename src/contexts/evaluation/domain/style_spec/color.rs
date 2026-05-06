use nu_ansi_term::Color;

#[derive(Debug, PartialEq, Eq)]
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

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ColorSpec {
    Named(NamedColor),
    Ansi256(u8),
    Rgb(u8, u8, u8),
}

pub(super) fn parse_named_color(s: &str) -> Option<NamedColor> {
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

pub(super) fn parse_color_value(s: &str) -> Option<ColorSpec> {
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

pub(super) fn color_spec_to_nu(spec: &ColorSpec) -> Color {
    match spec {
        ColorSpec::Named(n) => named_color_to_nu(n),
        ColorSpec::Ansi256(n) => Color::Fixed(*n),
        ColorSpec::Rgb(r, g, b) => Color::Rgb(*r, *g, *b),
    }
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
