use crate::tools::ToolState;
use crate::ui::theme::{Spinners, Theme};
use ratatui::style::Style;

#[must_use]
pub const fn state_style(state: &ToolState) -> Style {
    match state {
        ToolState::Starting | ToolState::InProgress => Theme::primary(),
        ToolState::Success => Theme::success(),
        ToolState::Error => Theme::error(),
    }
}

#[must_use]
pub fn state_spinner_frame(state: &ToolState, frame_index: usize) -> &'static str {
    match state {
        ToolState::Starting | ToolState::InProgress => {
            let frames = Spinners::CIRCLES;
            frames[frame_index % frames.len()]
        }
        ToolState::Success => "ok",
        ToolState::Error => "err",
    }
}
