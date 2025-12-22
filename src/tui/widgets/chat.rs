use crate::tools::{ToolState, ToolType};
use crate::ui::diff_widget::DiffWidget;
use crate::ui::markdown_widget::MarkdownWidget;
use crate::ui::output_widget::MessageLevel;
use crate::ui::theme::Theme;
use crate::ui::tool_card::ToolCard;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use std::time::Duration;

#[derive(Debug, Clone)]
pub enum ChatMessage {
    User(String),
    Assistant(String),
    System {
        text: String,
        level: MessageLevel,
    },
    StreamingAssistant(String),
    ToolExecution {
        tool_type: ToolType,
        input: String,
        output: Option<String>,
        elapsed: Option<Duration>,
        state: ToolState,
    },
    FileDiff {
        path: String,
        old_content: String,
        new_content: String,
        collapsed: bool,
    },
}

impl ChatMessage {
    pub fn render_to_lines(&self, width: u16, spinner_frame: usize) -> Vec<Line<'static>> {
        match self {
            Self::User(text) => Self::render_user_lines(text, width),
            Self::Assistant(text) => Self::render_assistant_lines(text, width),
            Self::System { text, level } => Self::render_system_lines(text, level),
            Self::StreamingAssistant(text) => Self::render_streaming_lines(text, width),
            Self::ToolExecution {
                tool_type,
                input,
                output,
                elapsed,
                state,
            } => Self::render_tool_lines(
                tool_type,
                input,
                output.as_ref(),
                *elapsed,
                state,
                width,
                spinner_frame,
            ),
            Self::FileDiff {
                path,
                old_content,
                new_content,
                collapsed,
            } => Self::render_diff_lines(path, old_content, new_content, *collapsed, width),
        }
    }

    fn render_user_lines(text: &str, width: u16) -> Vec<Line<'static>> {
        let prefix = "> ";
        let prefix_len = prefix.len();
        let available_width = (width as usize).saturating_sub(prefix_len + 1);

        let wrapped = textwrap::wrap(text, available_width);
        wrapped
            .into_iter()
            .enumerate()
            .map(|(i, line)| {
                if i == 0 {
                    Line::from(vec![
                        Span::styled(prefix.to_string(), Theme::white()),
                        Span::styled(line.to_string(), Theme::white()),
                    ])
                } else {
                    Line::from(Span::styled(
                        format!("{}{}", " ".repeat(prefix_len), line),
                        Theme::white(),
                    ))
                }
            })
            .collect()
    }

    fn render_assistant_lines(text: &str, width: u16) -> Vec<Line<'static>> {
        let markdown_widget = MarkdownWidget::new(text).indent(0).width(width as usize);

        if let Ok(mut markdown_lines) = markdown_widget.render_to_lines() {
            while markdown_lines.last().is_some_and(|l| l.spans.is_empty()) {
                markdown_lines.pop();
            }

            let max_lines = 50;
            let truncated = markdown_lines.len() > max_lines;
            markdown_lines.truncate(max_lines);

            let mut lines: Vec<Line<'static>> = markdown_lines
                .into_iter()
                .enumerate()
                .map(|(i, line)| {
                    if i == 0 {
                        let mut spans = vec![Span::styled("● ", Theme::off_white())];
                        for span in line.spans {
                            spans.push(Span::styled(span.content, Theme::off_white()));
                        }
                        Line::from(spans)
                    } else {
                        let mut spans = vec![Span::styled("  ", Theme::off_white())];
                        for span in line.spans {
                            spans.push(Span::styled(span.content, Theme::off_white()));
                        }
                        Line::from(spans)
                    }
                })
                .collect();

            if truncated {
                lines.push(Line::from(Span::styled(
                    "  ... (message truncated)",
                    Theme::muted(),
                )));
            }
            lines
        } else {
            let wrapped = textwrap::wrap(text, (width as usize).saturating_sub(4));
            let mut lines = Vec::new();
            for (i, line) in wrapped.iter().take(50).enumerate() {
                if i == 0 {
                    lines.push(Line::from(vec![
                        Span::styled("● ", Theme::off_white()),
                        Span::styled(line.to_string(), Theme::off_white()),
                    ]));
                } else {
                    lines.push(Line::from(Span::styled(
                        format!("  {line}"),
                        Theme::off_white(),
                    )));
                }
            }
            lines
        }
    }

    fn render_system_lines(text: &str, level: &MessageLevel) -> Vec<Line<'static>> {
        let icon = level.icon();
        let style = level.style();

        vec![Line::from(vec![
            Span::styled(format!("{icon} "), style),
            Span::styled(text.to_string(), style),
        ])]
    }

    fn render_streaming_lines(text: &str, width: u16) -> Vec<Line<'static>> {
        let markdown_widget = MarkdownWidget::new(text).indent(0).width(width as usize);

        let mut lines: Vec<Line<'static>> =
            if let Ok(mut markdown_lines) = markdown_widget.render_to_lines() {
                while markdown_lines.last().is_some_and(|l| l.spans.is_empty()) {
                    markdown_lines.pop();
                }

                markdown_lines
                    .into_iter()
                    .enumerate()
                    .map(|(i, line)| {
                        if i == 0 {
                            let mut spans = vec![Span::styled("● ", Theme::off_white())];
                            for span in line.spans {
                                spans.push(Span::styled(span.content, Theme::off_white()));
                            }
                            Line::from(spans)
                        } else {
                            let mut spans = vec![Span::styled("  ", Theme::off_white())];
                            for span in line.spans {
                                spans.push(Span::styled(span.content, Theme::off_white()));
                            }
                            Line::from(spans)
                        }
                    })
                    .collect()
            } else {
                let wrapped = textwrap::wrap(text, (width as usize).saturating_sub(4));
                wrapped
                    .iter()
                    .enumerate()
                    .map(|(i, line)| {
                        if i == 0 {
                            Line::from(vec![
                                Span::styled("● ", Theme::off_white()),
                                Span::styled(line.to_string(), Theme::off_white()),
                            ])
                        } else {
                            Line::from(Span::styled(format!("  {line}"), Theme::off_white()))
                        }
                    })
                    .collect()
            };

        if lines.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("● ", Theme::off_white()),
                Span::styled("▊", Theme::primary()),
            ]));
        } else if let Some(last_line) = lines.last_mut() {
            last_line.spans.push(Span::styled("▊", Theme::primary()));
        }

        lines
    }

    fn render_tool_lines(
        tool_type: &ToolType,
        input: &str,
        output: Option<&String>,
        elapsed: Option<Duration>,
        state: &ToolState,
        width: u16,
        spinner_frame: usize,
    ) -> Vec<Line<'static>> {
        let output_lines = if let Some(output_text) = output {
            output_text.lines().map(String::from).collect()
        } else {
            Vec::new()
        };

        let mut card = ToolCard::new(tool_type.clone(), input)
            .state(state.clone())
            .output(output_lines)
            .frame_index(spinner_frame);

        if let Some(elapsed_time) = elapsed {
            card = card.elapsed(elapsed_time);
        }

        card.render_to_lines(width)
    }

    fn render_diff_lines(
        path: &str,
        old_content: &str,
        new_content: &str,
        collapsed: bool,
        width: u16,
    ) -> Vec<Line<'static>> {
        let diff_widget = DiffWidget::new(path, old_content, new_content).collapsed(collapsed);
        diff_widget.render_to_lines(width)
    }
}

#[derive(Debug, Clone)]
pub struct ScrollState {
    position: usize,
    total_lines: usize,
    viewport_height: usize,
    manual_scroll: bool,
}

impl ScrollState {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            position: 0,
            total_lines: 0,
            viewport_height: 0,
            manual_scroll: false,
        }
    }

    #[must_use]
    pub const fn position(&self) -> usize {
        self.position
    }

    pub fn update(&mut self, total_lines: usize, viewport_height: usize) {
        self.total_lines = total_lines;
        self.viewport_height = viewport_height;

        self.position = self.position.min(self.max_scroll());
    }

    #[must_use]
    pub const fn is_at_bottom(&self) -> bool {
        if self.total_lines <= self.viewport_height {
            true
        } else {
            self.position >= self.max_scroll()
        }
    }

    pub const fn scroll_to_bottom(&mut self) {
        self.position = self.max_scroll();
        self.manual_scroll = false;
    }

    pub const fn scroll_up(&mut self, lines: usize) {
        self.position = self.position.saturating_sub(lines);
        self.manual_scroll = true;
    }

    pub fn scroll_down(&mut self, lines: usize) {
        self.position = (self.position + lines).min(self.max_scroll());
        self.manual_scroll = true;
    }

    pub const fn scroll_to_top(&mut self) {
        self.position = 0;
        self.manual_scroll = true;
    }

    const fn max_scroll(&self) -> usize {
        self.total_lines.saturating_sub(self.viewport_height)
    }

    pub const fn reset_manual_scroll(&mut self) {
        self.manual_scroll = false;
    }

    #[must_use]
    pub const fn is_manual_scroll(&self) -> bool {
        self.manual_scroll
    }
}

impl Default for ScrollState {
    fn default() -> Self {
        Self::new()
    }
}

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

        let mut all_lines: Vec<Line<'static>> = Vec::new();
        for (idx, message) in self.messages.iter().enumerate() {
            all_lines.extend(message.render_to_lines(content_width, self.spinner_frame));
            if idx < self.messages.len() - 1 {
                all_lines.push(Line::from(""));
            }
        }

        let total_lines = all_lines.len();
        self.scroll.update(total_lines, area.height as usize);

        if !self.scroll.is_manual_scroll() {
            self.scroll.scroll_to_bottom();
        }

        let offset = self.scroll.position();
        let viewport_height = area.height as usize;
        let end = (offset + viewport_height).min(total_lines);

        for (i, line) in all_lines[offset..end].iter().enumerate() {
            buf.set_line(area.x + 2, area.y + i as u16, line, content_width);
        }

        if !self.scroll.is_at_bottom() {
            Self::render_scroll_indicator(area, buf);
        }
    }

    fn render_empty_state(area: Rect, buf: &mut Buffer) {
        let message = vec![
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

        let block = Block::default().borders(Borders::NONE);
        let paragraph = Paragraph::new(message).block(block);
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
        let paragraph = Paragraph::new(indicator);
        paragraph.render(indicator_area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scroll_state_creation() {
        let state = ScrollState::new();
        assert_eq!(state.position(), 0);
        assert!(!state.is_manual_scroll());
    }

    #[test]
    fn test_scroll_state_is_at_bottom() {
        let mut state = ScrollState::new();
        state.update(10, 5);

        assert!(!state.is_at_bottom());

        state.scroll_to_bottom();
        assert!(state.is_at_bottom());
    }

    #[test]
    fn test_scroll_state_up_down() {
        let mut state = ScrollState::new();
        state.update(20, 10);

        state.scroll_down(5);
        assert_eq!(state.position(), 5);
        assert!(state.is_manual_scroll());

        state.scroll_up(2);
        assert_eq!(state.position(), 3);
    }

    #[test]
    fn test_scroll_state_clamp() {
        let mut state = ScrollState::new();
        state.update(20, 10);

        state.scroll_down(100);
        assert_eq!(state.position(), 10);

        state.scroll_up(100);
        assert_eq!(state.position(), 0);
    }

    #[test]
    fn test_chat_message_types() {
        use crate::ui::output_widget::MessageLevel;

        let user_msg = ChatMessage::User("Hello".to_string());
        let assistant_msg = ChatMessage::Assistant("Hi there!".to_string());
        let system_msg = ChatMessage::System {
            text: "System info".to_string(),
            level: MessageLevel::Info,
        };
        let diff_msg = ChatMessage::FileDiff {
            path: "/path/to/file.rs".to_string(),
            old_content: "old content".to_string(),
            new_content: "new content".to_string(),
            collapsed: false,
        };

        assert!(matches!(user_msg, ChatMessage::User(_)));
        assert!(matches!(assistant_msg, ChatMessage::Assistant(_)));
        assert!(matches!(system_msg, ChatMessage::System { .. }));
        assert!(matches!(diff_msg, ChatMessage::FileDiff { .. }));
    }

    #[test]
    fn test_file_diff_render_to_lines() {
        let diff_msg = ChatMessage::FileDiff {
            path: "/path/to/file.rs".to_string(),
            old_content: "line1\nline2\n".to_string(),
            new_content: "line1\nmodified\nline3\n".to_string(),
            collapsed: false,
        };

        let width = 80;
        let lines = diff_msg.render_to_lines(width, 0);

        assert!(!lines.is_empty());

        let collapsed_msg = ChatMessage::FileDiff {
            path: "/path/to/file.rs".to_string(),
            old_content: "line1\nline2\n".to_string(),
            new_content: "line1\nmodified\nline3\n".to_string(),
            collapsed: true,
        };
        let collapsed_lines = collapsed_msg.render_to_lines(width, 0);
        assert!(collapsed_lines.len() < lines.len());
    }

    #[test]
    fn test_assistant_markdown_rendering() {
        let markdown_text = "# Hello\n\nThis is **bold** and `code`.";
        let message = ChatMessage::Assistant(markdown_text.to_string());

        let lines = message.render_to_lines(80, 0);

        assert!(lines.len() > 1);
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_streaming_assistant_markdown() {
        let markdown_text = "## Section\n\n- Item 1\n- Item 2";
        let message = ChatMessage::StreamingAssistant(markdown_text.to_string());

        let lines = message.render_to_lines(80, 0);

        assert!(lines.len() > 1);

        if let Some(last_line) = lines.last() {
            let has_cursor = last_line
                .spans
                .iter()
                .any(|span| span.content.contains("▊"));
            assert!(has_cursor, "Streaming message should have cursor indicator");
        }
    }
}
