#![allow(clippy::cast_sign_loss)]

mod model_picker;
mod permission;

pub use model_picker::render_model_picker_modal;
pub use permission::render_permission_modal;

use crate::ui::theme::Theme;
use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

fn calc_centered_modal(area: Rect, width_ratio: f32, min: f32, max: f32, height: u16) -> Rect {
    let modal_width = (f32::from(area.width) * width_ratio).clamp(min, max) as u16;
    let modal_x = (area.width.saturating_sub(modal_width)) / 2;
    let modal_y = (area.height.saturating_sub(height)) / 2;
    Rect {
        x: modal_x,
        y: modal_y,
        width: modal_width,
        height,
    }
}

fn create_modal_block(title: &str, border_style: Style) -> Block<'static> {
    Block::default()
        .title(format!(" {title} "))
        .title_style(Theme::primary_bold())
        .borders(Borders::ALL)
        .border_style(border_style)
        .border_set(ratatui::symbols::border::ROUNDED)
}

fn render_hint(frame: &mut Frame, area: Rect, hint: &str) {
    let hint_line = Line::from(Span::styled(hint, Theme::muted()));
    frame.render_widget(Paragraph::new(hint_line).alignment(Alignment::Center), area);
}
