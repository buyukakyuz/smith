use crate::tools::{ToolState, ToolType};
use crate::ui::theme::{BoxChars, Spinners, Theme};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Widget,
};
use std::time::Duration;
use textwrap::wrap;
use unicode_width::UnicodeWidthStr;

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

#[derive(Debug, Clone)]
pub struct ToolCard {
    tool_type: ToolType,
    state: ToolState,
    input_summary: String,
    output_lines: Vec<String>,
    elapsed: Option<Duration>,
    frame_index: usize,
    show_preview: bool,
    max_preview_lines: usize,
}

impl ToolCard {
    #[must_use]
    pub fn new(tool_type: ToolType, input_summary: impl Into<String>) -> Self {
        Self {
            tool_type,
            state: ToolState::Starting,
            input_summary: input_summary.into(),
            output_lines: Vec::new(),
            elapsed: None,
            frame_index: 0,
            show_preview: true,
            max_preview_lines: 5,
        }
    }

    #[must_use]
    pub const fn state(mut self, state: ToolState) -> Self {
        self.state = state;
        self
    }

    #[must_use]
    pub fn output(mut self, lines: Vec<String>) -> Self {
        self.output_lines = lines;
        self
    }

    #[must_use]
    pub const fn elapsed(mut self, elapsed: Duration) -> Self {
        self.elapsed = Some(elapsed);
        self
    }

    #[must_use]
    pub const fn frame_index(mut self, index: usize) -> Self {
        self.frame_index = index;
        self
    }
}

impl Widget for ToolCard {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 20 || area.height < 3 {
            return;
        }

        let ctx = RenderContext::new(&self, area);
        let mut y = area.y;

        y = ctx.render_top_border(buf, y);
        y = ctx.render_input_summary(buf, y);

        if self.should_show_preview() {
            y = ctx.render_preview_section(buf, y);
        }

        ctx.render_bottom_border(buf, y);
    }
}

impl ToolCard {
    fn should_show_preview(&self) -> bool {
        self.show_preview && !self.output_lines.is_empty() && self.state == ToolState::Success
    }

    #[must_use]
    pub fn render_to_lines(&self, width: u16) -> Vec<Line<'static>> {
        if width < 20 {
            return vec![];
        }

        let mut lines = Vec::new();
        let width_usize = width as usize;
        let content_width = width_usize.saturating_sub(4);
        let state_style = state_style(&self.state);

        lines.push(self.build_top_border_line(width_usize, state_style));

        lines.extend(self.build_input_lines(content_width, state_style));

        if self.should_show_preview() {
            lines.extend(self.build_preview_lines(content_width, state_style));
        }

        lines.push(Self::build_bottom_border_line(width_usize, state_style));

        lines
    }

    fn build_top_border_line(&self, width: usize, state_style: Style) -> Line<'static> {
        let title_text = format!(" {} {} ", self.spinner_frame(), self.tool_type.name());
        let elapsed_text = self.format_elapsed();

        let remaining = width
            .saturating_sub(2)
            .saturating_sub(title_text.width())
            .saturating_sub(elapsed_text.width());

        Line::from(vec![
            Span::styled(BoxChars::ROUND_TOP_LEFT.to_string(), state_style),
            Span::styled(BoxChars::HORIZONTAL.to_string(), state_style),
            Span::styled(title_text, state_style.add_modifier(Modifier::BOLD)),
            Span::styled(BoxChars::HORIZONTAL.repeat(remaining), state_style),
            Span::styled(elapsed_text, state_style),
            Span::styled(BoxChars::HORIZONTAL.to_string(), state_style),
            Span::styled(BoxChars::ROUND_TOP_RIGHT.to_string(), state_style),
        ])
    }

    fn format_elapsed(&self) -> String {
        self.elapsed.map_or_else(String::new, |elapsed| {
            let ms = elapsed.as_millis();
            if ms < 1000 {
                format!(" {ms}ms ")
            } else {
                format!(" {:.2}s ", elapsed.as_secs_f64())
            }
        })
    }

    fn build_input_lines(&self, content_width: usize, state_style: Style) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        let wrapped_input = wrap(&self.input_summary, content_width);

        for line_text in wrapped_input.iter().take(2) {
            let text = line_text.as_ref();
            let padding = content_width.saturating_sub(text.width());
            let line = Line::from(vec![
                Span::styled(BoxChars::VERTICAL.to_string(), state_style),
                Span::raw("  ".to_string()),
                Span::styled(text.to_string(), Style::default()),
                Span::raw(" ".repeat(padding)),
                Span::raw("  ".to_string()),
                Span::styled(BoxChars::VERTICAL.to_string(), state_style),
            ]);
            lines.push(line);
        }
        lines
    }

    fn build_preview_lines(&self, content_width: usize, state_style: Style) -> Vec<Line<'static>> {
        let mut lines = Vec::new();

        let divider = Line::from(vec![
            Span::styled(BoxChars::VERTICAL.to_string(), state_style),
            Span::styled(
                format!(" {}", BoxChars::DIVIDER_LIGHT.repeat(content_width + 2)),
                Theme::muted(),
            ),
            Span::styled(BoxChars::VERTICAL.to_string(), state_style),
        ]);
        lines.push(divider);

        for (i, line) in self
            .output_lines
            .iter()
            .enumerate()
            .take(self.max_preview_lines)
        {
            let line_num = format!("{:3} │ ", i + 1);
            let line_content = Self::truncate_line(line, line_num.width(), content_width);
            let padding = content_width.saturating_sub(line_num.width() + line_content.width());

            let preview_line = Line::from(vec![
                Span::styled(BoxChars::VERTICAL.to_string(), state_style),
                Span::raw("  ".to_string()),
                Span::styled(line_num, Theme::muted()),
                Span::styled(line_content, Theme::muted()),
                Span::raw(" ".repeat(padding)),
                Span::raw("  ".to_string()),
                Span::styled(BoxChars::VERTICAL.to_string(), state_style),
            ]);
            lines.push(preview_line);
        }

        let remaining = self
            .output_lines
            .len()
            .saturating_sub(self.max_preview_lines);

        if remaining > 0 {
            let more_text = format!("  ... ({remaining} more lines)");
            let padding = content_width.saturating_sub(more_text.width());

            let more_line = Line::from(vec![
                Span::styled(BoxChars::VERTICAL.to_string(), state_style),
                Span::raw("  ".to_string()),
                Span::styled(more_text, Theme::muted()),
                Span::raw(" ".repeat(padding)),
                Span::raw("  ".to_string()),
                Span::styled(BoxChars::VERTICAL.to_string(), state_style),
            ]);
            lines.push(more_line);
        }

        lines
    }

    fn truncate_line(line: &str, prefix_width: usize, content_width: usize) -> String {
        if line.width() + prefix_width > content_width {
            let max_len = content_width.saturating_sub(prefix_width).saturating_sub(3);
            format!("{}...", line.chars().take(max_len).collect::<String>())
        } else {
            line.to_string()
        }
    }

    fn build_bottom_border_line(width: usize, state_style: Style) -> Line<'static> {
        Line::from(vec![Span::styled(
            format!(
                "{}{}{}",
                BoxChars::ROUND_BOTTOM_LEFT,
                BoxChars::HORIZONTAL.repeat(width.saturating_sub(2)),
                BoxChars::ROUND_BOTTOM_RIGHT
            ),
            state_style,
        )])
    }
}

struct RenderContext<'a> {
    card: &'a ToolCard,
    area: Rect,
    width: usize,
    content_width: usize,
    state_style: Style,
}

impl<'a> RenderContext<'a> {
    const fn new(card: &'a ToolCard, area: Rect) -> Self {
        let width = area.width as usize;
        Self {
            card,
            area,
            width,
            content_width: width.saturating_sub(4),
            state_style: state_style(&card.state),
        }
    }

    fn render_top_border(&self, buf: &mut Buffer, y: u16) -> u16 {
        let title_text = format!(
            " {} {} ",
            self.card.spinner_frame(),
            self.card.tool_type.name()
        );
        let elapsed_text = self.card.format_elapsed();

        let remaining = self
            .width
            .saturating_sub(2)
            .saturating_sub(title_text.width())
            .saturating_sub(elapsed_text.width());

        let top_line = Line::from(vec![
            Span::styled(BoxChars::ROUND_TOP_LEFT, self.state_style),
            Span::styled(BoxChars::HORIZONTAL, self.state_style),
            Span::styled(title_text, self.state_style.add_modifier(Modifier::BOLD)),
            Span::styled(BoxChars::HORIZONTAL.repeat(remaining), self.state_style),
            Span::styled(elapsed_text, self.state_style),
            Span::styled(BoxChars::HORIZONTAL, self.state_style),
            Span::styled(BoxChars::ROUND_TOP_RIGHT, self.state_style),
        ]);

        buf.set_line(self.area.x, y, &top_line, self.area.width);
        y + 1
    }

    fn render_input_summary(&self, buf: &mut Buffer, mut y: u16) -> u16 {
        let wrapped_input = wrap(&self.card.input_summary, self.content_width);

        for line_text in wrapped_input.iter().take(2) {
            if y >= self.area.y + self.area.height - 1 {
                break;
            }
            self.render_content_line(buf, y, line_text.as_ref(), Style::default());
            y += 1;
        }
        y
    }

    fn render_content_line(&self, buf: &mut Buffer, y: u16, text: &str, style: Style) {
        let padding = self.content_width.saturating_sub(text.width());
        let line = Line::from(vec![
            Span::styled(BoxChars::VERTICAL, self.state_style),
            Span::raw("  "),
            Span::styled(text.to_string(), style),
            Span::raw(" ".repeat(padding)),
            Span::raw("  "),
            Span::styled(BoxChars::VERTICAL, self.state_style),
        ]);
        buf.set_line(self.area.x, y, &line, self.area.width);
    }

    fn render_preview_section(&self, buf: &mut Buffer, y: u16) -> u16 {
        let y = self.render_divider(buf, y);
        let y = self.render_preview_lines(buf, y);
        self.render_more_indicator(buf, y)
    }

    fn render_divider(&self, buf: &mut Buffer, y: u16) -> u16 {
        if y >= self.area.y + self.area.height - 1 {
            return y;
        }

        let divider = Line::from(vec![
            Span::styled(BoxChars::VERTICAL, self.state_style),
            Span::styled(
                format!(
                    " {}",
                    BoxChars::DIVIDER_LIGHT.repeat(self.content_width + 2)
                ),
                Theme::muted(),
            ),
            Span::styled(BoxChars::VERTICAL, self.state_style),
        ]);
        buf.set_line(self.area.x, y, &divider, self.area.width);
        y + 1
    }

    fn render_preview_lines(&self, buf: &mut Buffer, mut y: u16) -> u16 {
        for (i, line) in self
            .card
            .output_lines
            .iter()
            .enumerate()
            .take(self.card.max_preview_lines)
        {
            if y >= self.area.y + self.area.height - 1 {
                break;
            }
            self.render_numbered_line(buf, y, i + 1, line);
            y += 1;
        }
        y
    }

    fn render_numbered_line(&self, buf: &mut Buffer, y: u16, num: usize, line: &str) {
        let line_num = format!("{num:3} │ ");
        let line_content = ToolCard::truncate_line(line, line_num.width(), self.content_width);
        let padding = self
            .content_width
            .saturating_sub(line_num.width() + line_content.width());

        let preview_line = Line::from(vec![
            Span::styled(BoxChars::VERTICAL, self.state_style),
            Span::raw("  "),
            Span::styled(line_num, Theme::muted()),
            Span::styled(line_content, Theme::muted()),
            Span::raw(" ".repeat(padding)),
            Span::raw("  "),
            Span::styled(BoxChars::VERTICAL, self.state_style),
        ]);
        buf.set_line(self.area.x, y, &preview_line, self.area.width);
    }

    fn render_more_indicator(&self, buf: &mut Buffer, y: u16) -> u16 {
        let remaining = self
            .card
            .output_lines
            .len()
            .saturating_sub(self.card.max_preview_lines);

        if remaining == 0 || y >= self.area.y + self.area.height - 1 {
            return y;
        }

        let more_text = format!("  ... ({remaining} more lines)");
        let padding = self.content_width.saturating_sub(more_text.width());

        let more_line = Line::from(vec![
            Span::styled(BoxChars::VERTICAL, self.state_style),
            Span::raw("  "),
            Span::styled(more_text, Theme::muted()),
            Span::raw(" ".repeat(padding)),
            Span::raw("  "),
            Span::styled(BoxChars::VERTICAL, self.state_style),
        ]);
        buf.set_line(self.area.x, y, &more_line, self.area.width);
        y + 1
    }

    fn render_bottom_border(&self, buf: &mut Buffer, y: u16) {
        if y >= self.area.y + self.area.height {
            return;
        }

        let bottom_line = Line::from(vec![Span::styled(
            format!(
                "{}{}{}",
                BoxChars::ROUND_BOTTOM_LEFT,
                BoxChars::HORIZONTAL.repeat(self.width.saturating_sub(2)),
                BoxChars::ROUND_BOTTOM_RIGHT
            ),
            self.state_style,
        )]);
        buf.set_line(self.area.x, y, &bottom_line, self.area.width);
    }
}

impl ToolCard {
    fn spinner_frame(&self) -> &'static str {
        state_spinner_frame(&self.state, self.frame_index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_state() {
        assert_eq!(state_spinner_frame(&ToolState::Success, 0), "ok");
        assert_eq!(state_spinner_frame(&ToolState::Error, 0), "err");
    }

    #[test]
    fn test_tool_type() {
        assert_eq!(ToolType::ReadFile.name(), "read_file");
        assert_eq!(ToolType::Bash.name(), "bash");
        assert_eq!(ToolType::Custom("test".to_string()).name(), "test");
    }

    #[test]
    fn test_tool_card_creation() {
        let card = ToolCard::new(ToolType::ReadFile, "main.rs")
            .state(ToolState::Success)
            .elapsed(Duration::from_millis(127))
            .output(vec!["line 1".to_string(), "line 2".to_string()]);

        assert_eq!(card.state, ToolState::Success);
        assert_eq!(card.output_lines.len(), 2);
    }

    #[test]
    fn test_render_to_lines() {
        let card = ToolCard::new(ToolType::ReadFile, "test.rs")
            .state(ToolState::Success)
            .elapsed(Duration::from_millis(42))
            .output(vec!["line 1".to_string(), "line 2".to_string()]);

        let lines = card.render_to_lines(80);

        assert_eq!(lines.len(), 6);

        let lines_narrow = card.render_to_lines(10);
        assert_eq!(lines_narrow.len(), 0);

        let card_no_preview =
            ToolCard::new(ToolType::ReadFile, "test.rs").state(ToolState::InProgress);
        let lines_no_preview = card_no_preview.render_to_lines(80);

        assert_eq!(lines_no_preview.len(), 3);
    }
}
