use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};
use textwrap::wrap;
use unicode_width::UnicodeWidthStr;

use crate::ui::theme::{BoxChars, Theme};

use super::card::ToolCard;
use super::state::{state_spinner_frame, state_style};

const MIN_WIDTH: u16 = 20;

impl ToolCard {
    #[must_use]
    pub fn render_to_lines(&self, width: u16) -> Vec<Line<'static>> {
        if width < MIN_WIDTH {
            return vec![];
        }

        let ctx = LineContext::new(self, width);
        let mut lines = Vec::new();

        lines.push(ctx.top_border());
        lines.extend(ctx.input_lines());

        if self.should_show_preview() {
            lines.push(ctx.divider());
            lines.extend(ctx.preview_lines());
            if let Some(more) = ctx.more_indicator() {
                lines.push(more);
            }
        }

        lines.push(ctx.bottom_border());
        lines
    }
}

struct LineContext<'a> {
    card: &'a ToolCard,
    width: usize,
    content_width: usize,
    style: Style,
}

impl<'a> LineContext<'a> {
    fn new(card: &'a ToolCard, width: u16) -> Self {
        let width = width as usize;
        Self {
            card,
            width,
            content_width: width.saturating_sub(4),
            style: state_style(&card.state),
        }
    }

    fn top_border(&self) -> Line<'static> {
        let title = format!(
            " {} {} ",
            state_spinner_frame(&self.card.state, self.card.frame_index),
            self.card.tool_type.name()
        );
        let elapsed = self.format_elapsed();

        let fill_width = self
            .width
            .saturating_sub(2)
            .saturating_sub(title.width())
            .saturating_sub(elapsed.width());

        Line::from(vec![
            Span::styled(BoxChars::ROUND_TOP_LEFT.to_string(), self.style),
            Span::styled(BoxChars::HORIZONTAL.to_string(), self.style),
            Span::styled(title, self.style.add_modifier(Modifier::BOLD)),
            Span::styled(BoxChars::HORIZONTAL.repeat(fill_width), self.style),
            Span::styled(elapsed, self.style),
            Span::styled(BoxChars::HORIZONTAL.to_string(), self.style),
            Span::styled(BoxChars::ROUND_TOP_RIGHT.to_string(), self.style),
        ])
    }

    fn bottom_border(&self) -> Line<'static> {
        Line::from(vec![Span::styled(
            format!(
                "{}{}{}",
                BoxChars::ROUND_BOTTOM_LEFT,
                BoxChars::HORIZONTAL.repeat(self.width.saturating_sub(2)),
                BoxChars::ROUND_BOTTOM_RIGHT
            ),
            self.style,
        )])
    }

    fn divider(&self) -> Line<'static> {
        Line::from(vec![
            Span::styled(BoxChars::VERTICAL.to_string(), self.style),
            Span::styled(
                format!(
                    " {}",
                    BoxChars::DIVIDER_LIGHT.repeat(self.content_width + 2)
                ),
                Theme::muted(),
            ),
            Span::styled(BoxChars::VERTICAL.to_string(), self.style),
        ])
    }

    fn input_lines(&self) -> Vec<Line<'static>> {
        wrap(&self.card.input_summary, self.content_width)
            .into_iter()
            .take(2)
            .map(|text| self.content_line(text.as_ref(), Style::default()))
            .collect()
    }

    fn preview_lines(&self) -> Vec<Line<'static>> {
        self.card
            .output_lines
            .iter()
            .enumerate()
            .take(self.card.max_preview_lines)
            .map(|(i, line)| self.numbered_line(i + 1, line))
            .collect()
    }

    fn more_indicator(&self) -> Option<Line<'static>> {
        let remaining = self
            .card
            .output_lines
            .len()
            .saturating_sub(self.card.max_preview_lines);

        if remaining == 0 {
            return None;
        }

        let text = format!("  ... ({remaining} more lines)");
        Some(self.content_line(&text, Theme::muted()))
    }

    fn content_line(&self, text: &str, text_style: Style) -> Line<'static> {
        let padding = self.content_width.saturating_sub(text.width());
        Line::from(vec![
            Span::styled(BoxChars::VERTICAL.to_string(), self.style),
            Span::raw("  ".to_string()),
            Span::styled(text.to_string(), text_style),
            Span::raw(" ".repeat(padding)),
            Span::raw("  ".to_string()),
            Span::styled(BoxChars::VERTICAL.to_string(), self.style),
        ])
    }

    fn numbered_line(&self, num: usize, line: &str) -> Line<'static> {
        let prefix = format!("{num:3} │ ");
        let content = Self::truncate(line, prefix.width(), self.content_width);
        let padding = self
            .content_width
            .saturating_sub(prefix.width() + content.width());

        Line::from(vec![
            Span::styled(BoxChars::VERTICAL.to_string(), self.style),
            Span::raw("  ".to_string()),
            Span::styled(prefix, Theme::muted()),
            Span::styled(content, Theme::muted()),
            Span::raw(" ".repeat(padding)),
            Span::raw("  ".to_string()),
            Span::styled(BoxChars::VERTICAL.to_string(), self.style),
        ])
    }

    fn format_elapsed(&self) -> String {
        self.card.elapsed.map_or_else(String::new, |elapsed| {
            let ms = elapsed.as_millis();
            if ms < 1000 {
                format!(" {ms}ms ")
            } else {
                format!(" {:.2}s ", elapsed.as_secs_f64())
            }
        })
    }

    fn truncate(line: &str, prefix_width: usize, content_width: usize) -> String {
        let max_width = content_width.saturating_sub(prefix_width);
        if line.width() <= max_width {
            line.to_string()
        } else {
            let truncated: String = line.chars().take(max_width.saturating_sub(3)).collect();
            format!("{truncated}...")
        }
    }
}
