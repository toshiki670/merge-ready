use bitflags::bitflags;
use nu_ansi_term::Style;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub(crate) struct StyleModifiers: u16 {
        const BOLD = 1 << 0;
        const ITALIC = 1 << 1;
        const UNDERLINE = 1 << 2;
        const DIMMED = 1 << 3;
        const INVERTED = 1 << 4;
        const BLINK = 1 << 5;
        const HIDDEN = 1 << 6;
        const STRIKETHROUGH = 1 << 7;
    }
}

impl StyleModifiers {
    pub(super) fn parse_token(token: &str) -> Option<Self> {
        match token {
            "bold" => Some(Self::BOLD),
            "italic" => Some(Self::ITALIC),
            "underline" => Some(Self::UNDERLINE),
            "dimmed" => Some(Self::DIMMED),
            "inverted" => Some(Self::INVERTED),
            "blink" => Some(Self::BLINK),
            "hidden" => Some(Self::HIDDEN),
            "strikethrough" => Some(Self::STRIKETHROUGH),
            _ => None,
        }
    }

    pub(super) fn apply_to(self, mut style: Style) -> Style {
        if self.contains(Self::BOLD) {
            style = style.bold();
        }
        if self.contains(Self::ITALIC) {
            style = style.italic();
        }
        if self.contains(Self::UNDERLINE) {
            style = style.underline();
        }
        if self.contains(Self::DIMMED) {
            style = style.dimmed();
        }
        if self.contains(Self::INVERTED) {
            style = style.reverse();
        }
        if self.contains(Self::BLINK) {
            style = style.blink();
        }
        if self.contains(Self::HIDDEN) {
            style = style.hidden();
        }
        if self.contains(Self::STRIKETHROUGH) {
            style = style.strikethrough();
        }
        style
    }
}
