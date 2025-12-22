use crate::ui::theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageLevel {
    Info,
    Error,
}

impl MessageLevel {
    #[must_use]
    pub const fn icon(&self) -> &'static str {
        match self {
            Self::Info => "[i]",
            Self::Error => "[x]",
        }
    }

    #[must_use]
    pub const fn style(&self) -> ratatui::style::Style {
        match self {
            Self::Info => Theme::primary(),
            Self::Error => Theme::error(),
        }
    }
}
