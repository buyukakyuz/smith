mod message;
mod render;
mod scroll_state;

pub use message::ChatMessage;
pub use scroll_state::ScrollState;

use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::ui::theme::Theme;

pub struct ChatWidget<'a> {
    messages: &'a [ChatMessage],
    scroll: &'a mut ScrollState,
    spinner_frame: usize,
}

impl<'a> ChatWidget<'a> {
    #[must_use]
    pub const fn new(
        messages: &'a [ChatMessage],
        scroll: &'a mut ScrollState,
        spinner_frame: usize,
    ) -> Self {
        Self {
            messages,
            scroll,
            spinner_frame,
        }
    }

    pub fn render(self, area: Rect, buf: &mut Buffer) {
        if self.messages.is_empty() {
            Self::render_empty_state(area, buf);
            return;
        }

        let content_width = area.width.saturating_sub(4);
        let all_lines = self.collect_all_lines(content_width);

        self.update_scroll_and_render(area, buf, all_lines, content_width);
    }

    fn collect_all_lines(&self, width: u16) -> Vec<Line<'static>> {
        let mut lines = Vec::new();

        for (idx, message) in self.messages.iter().enumerate() {
            lines.extend(message.render_to_lines(width, self.spinner_frame));

            if idx < self.messages.len() - 1 {
                lines.push(Line::from(""));
            }
        }

        lines
    }

    fn update_scroll_and_render(
        self,
        area: Rect,
        buf: &mut Buffer,
        lines: Vec<Line<'static>>,
        content_width: u16,
    ) {
        let total_lines = lines.len();
        let viewport_height = area.height as usize;

        self.scroll.update(total_lines, viewport_height);

        if !self.scroll.is_manual_scroll() {
            self.scroll.scroll_to_bottom();
        }

        let offset = self.scroll.position();
        let end = (offset + viewport_height).min(total_lines);

        for (i, line) in lines[offset..end].iter().enumerate() {
            buf.set_line(area.x + 2, area.y + i as u16, line, content_width);
        }

        if !self.scroll.is_at_bottom() {
            Self::render_scroll_indicator(area, buf);
        }
    }

    fn render_empty_state(area: Rect, buf: &mut Buffer) {
        let lines = vec![
            Line::from(""),
            Line::from(Span::styled("Welcome to Smith!", Theme::primary_bold()))
                .alignment(Alignment::Center),
            Line::from(""),
            Line::from(Span::styled(
                "Type your message below and press Enter to chat with the AI.",
                Theme::muted(),
            ))
            .alignment(Alignment::Center),
            Line::from(""),
            Line::from(Span::styled("Press Ctrl+C to exit", Theme::muted()))
                .alignment(Alignment::Center),
        ];

        let paragraph = Paragraph::new(lines).block(Block::default().borders(Borders::NONE));
        paragraph.render(area, buf);
    }

    fn render_scroll_indicator(area: Rect, buf: &mut Buffer) {
        let indicator_area = Rect {
            x: area.x + area.width - 10,
            y: area.y + area.height - 1,
            width: 10,
            height: 1,
        };

        let indicator = Line::from(Span::styled("↓ More", Theme::warning()));
        Paragraph::new(indicator).render(indicator_area, buf);
    }
}
