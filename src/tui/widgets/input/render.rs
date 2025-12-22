#![allow(clippy::cast_possible_truncation)]

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use super::InputWidget;
use crate::ui::theme::Theme;

impl InputWidget<'_> {
    pub fn render(&mut self, area: Rect, frame: &mut Frame) {
        self.render_separator(area, frame.buffer_mut());

        let input_area = self.input_area(area);
        self.render_prefix(input_area, frame.buffer_mut());
        self.render_hint_if_needed(input_area, frame.buffer_mut());

        let textarea_area = self.textarea_area(input_area);
        self.render_textarea(textarea_area, frame);

        if self.show_suggestions && !self.suggestions.is_empty() {
            self.render_suggestions(area, frame.buffer_mut());
        }
    }

    fn render_separator(&self, area: Rect, buf: &mut Buffer) {
        let separator_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 1,
        };

        let line = Line::from(Span::styled(
            "─".repeat(area.width as usize),
            Theme::border(),
        ));
        Paragraph::new(line).render(separator_area, buf);
    }

    fn render_prefix(&self, input_area: Rect, buf: &mut Buffer) {
        let prefix_area = Rect {
            x: input_area.x,
            y: input_area.y,
            width: 2,
            height: 1,
        };

        let prefix = Line::from(Span::styled("> ", Theme::white()));
        Paragraph::new(prefix).render(prefix_area, buf);
    }

    fn render_hint_if_needed(&self, input_area: Rect, buf: &mut Buffer) {
        if self.is_empty() {
            return;
        }

        const HINT_TEXT: &str = "↵ send";
        let hint_width = HINT_TEXT.len() as u16;

        let hint_area = Rect {
            x: input_area.x + input_area.width - hint_width - 1,
            y: input_area.y,
            width: hint_width,
            height: 1,
        };

        let hint = Line::from(Span::styled(HINT_TEXT, Theme::muted()));
        Paragraph::new(hint).render(hint_area, buf);
    }

    fn render_textarea(&mut self, area: Rect, frame: &mut Frame) {
        self.textarea
            .set_block(Block::default().borders(Borders::NONE));

        frame.render_widget(&self.textarea, area);

        let (cursor_row, cursor_col) = self.textarea.cursor();
        frame.set_cursor_position(Position::new(
            area.x + cursor_col as u16,
            area.y + cursor_row as u16,
        ));
    }

    fn render_suggestions(&self, area: Rect, buf: &mut Buffer) {
        let height = self.suggestions.len().min(5) as u16 + 2;

        if area.y < height {
            return;
        }

        let suggestions_area = Rect {
            x: area.x + 2,
            y: area.y.saturating_sub(height),
            width: 30.min(area.width.saturating_sub(4)),
            height,
        };

        let lines: Vec<Line> = self
            .suggestions
            .iter()
            .enumerate()
            .map(|(i, cmd)| self.render_suggestion_line(i, cmd))
            .collect();

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Theme::primary())
            .border_set(ratatui::symbols::border::ROUNDED)
            .title(" Commands ");

        Paragraph::new(lines)
            .block(block)
            .render(suggestions_area, buf);
    }

    fn render_suggestion_line<'a>(&self, index: usize, command: &'a str) -> Line<'a> {
        let is_selected = index == self.selected_suggestion;

        let style = if is_selected {
            Theme::primary_bold()
        } else {
            Theme::muted()
        };

        let indicator = if is_selected {
            Span::styled(" ←", Theme::primary())
        } else {
            Span::raw("  ")
        };

        Line::from(vec![
            Span::raw(" "),
            Span::styled(command, style),
            indicator,
        ])
    }
    fn input_area(&self, area: Rect) -> Rect {
        Rect {
            x: area.x,
            y: area.y + 1,
            width: area.width,
            height: area.height.saturating_sub(1),
        }
    }

    fn textarea_area(&self, input_area: Rect) -> Rect {
        const HINT_WIDTH: u16 = 8;

        Rect {
            x: input_area.x + 2,
            y: input_area.y,
            width: input_area.width.saturating_sub(2 + HINT_WIDTH),
            height: input_area.height,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn textarea_area_leaves_room_for_hint() {
        let widget = InputWidget::new();
        let input_area = Rect::new(0, 0, 80, 3);

        let textarea = widget.textarea_area(input_area);

        assert!(textarea.width < input_area.width);
        assert_eq!(textarea.x, 2);
    }
}
