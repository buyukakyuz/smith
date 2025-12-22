use ratatui::style::{Color, Modifier, Style};
pub struct BrandColors;

impl BrandColors {
    pub const CYAN: Color = Color::Rgb(0, 217, 255);
    pub const PURPLE: Color = Color::Rgb(167, 139, 250);
    pub const GREEN: Color = Color::Rgb(16, 185, 129);
    pub const AMBER: Color = Color::Rgb(245, 158, 11);
    pub const RED: Color = Color::Rgb(239, 68, 68);
    pub const GRAY: Color = Color::Rgb(107, 114, 128);
    pub const DARK_GRAY: Color = Color::Rgb(55, 65, 81);
    pub const WHITE: Color = Color::Rgb(255, 255, 255);
    pub const OFF_WHITE: Color = Color::Rgb(184, 184, 184);
}

pub struct BoxChars;

impl BoxChars {
    pub const ROUND_TOP_LEFT: &'static str = "╭";
    pub const ROUND_TOP_RIGHT: &'static str = "╮";
    pub const ROUND_BOTTOM_LEFT: &'static str = "╰";
    pub const ROUND_BOTTOM_RIGHT: &'static str = "╯";
    pub const HORIZONTAL: &'static str = "─";
    pub const VERTICAL: &'static str = "│";
    pub const DIVIDER_LIGHT: &'static str = "┄";
    pub const DOT: &'static str = "•";
    pub const ELLIPSIS: &'static str = "⋮";
    pub const ARROW_RIGHT: &'static str = "❯";
}

pub struct Spinners;

impl Spinners {
    pub const BRAILLE: &'static [&'static str] =
        &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    pub const CIRCLES: &'static [&'static str] = &["◐", "◓", "◑", "◒"];
}

pub struct Theme;

impl Theme {
    #[must_use]
    pub const fn primary() -> Style {
        Style::new().fg(BrandColors::CYAN)
    }
    #[must_use]
    pub const fn secondary() -> Style {
        Style::new().fg(BrandColors::PURPLE)
    }
    #[must_use]
    pub const fn success() -> Style {
        Style::new().fg(BrandColors::GREEN)
    }
    #[must_use]
    pub const fn warning() -> Style {
        Style::new().fg(BrandColors::AMBER)
    }
    #[must_use]
    pub const fn error() -> Style {
        Style::new().fg(BrandColors::RED)
    }

    #[must_use]
    pub const fn muted() -> Style {
        Style::new().fg(BrandColors::GRAY)
    }

    #[must_use]
    pub const fn border() -> Style {
        Style::new().fg(BrandColors::DARK_GRAY)
    }

    #[must_use]
    pub const fn white() -> Style {
        Style::new().fg(BrandColors::WHITE)
    }

    #[must_use]
    pub const fn off_white() -> Style {
        Style::new().fg(BrandColors::OFF_WHITE)
    }

    #[must_use]
    pub const fn primary_bold() -> Style {
        Style::new()
            .fg(BrandColors::CYAN)
            .add_modifier(Modifier::BOLD)
    }
}
