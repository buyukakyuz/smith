use crate::ui::theme::Theme;
use ratatui::style::Style;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeType {
    Create,
    Update,
    Delete,
}

impl ChangeType {
    #[must_use]
    pub const fn icon(self) -> &'static str {
        match self {
            Self::Create => "+",
            Self::Update => "~",
            Self::Delete => "-",
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Create => "Create",
            Self::Update => "Update",
            Self::Delete => "Delete",
        }
    }

    #[must_use]
    pub const fn style(self) -> Style {
        match self {
            Self::Create => Theme::success(),
            Self::Update => Theme::primary(),
            Self::Delete => Theme::error(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiffLine {
    pub line_num: usize,
    pub tag: LineTag,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineTag {
    Added,
    Removed,
    Unchanged,
}

impl LineTag {
    #[must_use]
    pub const fn indicator(self) -> &'static str {
        match self {
            Self::Added => "+",
            Self::Removed => "-",
            Self::Unchanged => " ",
        }
    }

    #[must_use]
    pub const fn style(self) -> Style {
        match self {
            Self::Added => Theme::success(),
            Self::Removed => Theme::error(),
            Self::Unchanged => Style::new(),
        }
    }

    #[must_use]
    pub const fn is_change(self) -> bool {
        !matches!(self, Self::Unchanged)
    }
}
