use std::time::Duration;

use ratatui::text::{Line, Span};

use super::ChatMessage;
use crate::tools::{ToolState, ToolType};
use crate::ui::diff::DiffWidget;
use crate::ui::markdown::MarkdownWidget;
use crate::ui::output_widget::MessageLevel;
use crate::ui::theme::Theme;
use crate::ui::tool_card::ToolCard;

const MAX_MESSAGE_LINES: usize = 50;

impl ChatMessage {
    pub fn render_to_lines(&self, width: u16, spinner_frame: usize) -> Vec<Line<'static>> {
        match self {
            Self::User(text) => render_user(text, width),
            Self::Assistant(text) => render_assistant(text, width, false),
            Self::StreamingAssistant(text) => render_assistant(text, width, true),
            Self::System { text, level } => render_system(text, level),
            Self::ToolExecution {
                tool_type,
                input,
                output,
                elapsed,
                state,
            } => render_tool(
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
            } => render_diff(path, old_content, new_content, *collapsed, width),
        }
    }
}

fn render_user(text: &str, width: u16) -> Vec<Line<'static>> {
    const PREFIX: &str = "> ";

    let available_width = (width as usize).saturating_sub(PREFIX.len() + 1);
    let wrapped = textwrap::wrap(text, available_width);

    wrapped
        .into_iter()
        .enumerate()
        .map(|(i, line)| {
            if i == 0 {
                Line::from(vec![
                    Span::styled(PREFIX.to_string(), Theme::white()),
                    Span::styled(line.to_string(), Theme::white()),
                ])
            } else {
                Line::from(Span::styled(
                    format!("{:width$}{}", "", line, width = PREFIX.len()),
                    Theme::white(),
                ))
            }
        })
        .collect()
}

fn render_assistant(text: &str, width: u16, streaming: bool) -> Vec<Line<'static>> {
    let mut lines = render_markdown_with_prefix(text, width);

    if streaming {
        append_cursor(&mut lines);
    } else {
        truncate_with_indicator(&mut lines);
    }

    lines
}

fn render_system(text: &str, level: &MessageLevel) -> Vec<Line<'static>> {
    let icon = level.icon();
    let style = level.style();

    vec![Line::from(vec![
        Span::styled(format!("{icon} "), style),
        Span::styled(text.to_string(), style),
    ])]
}

fn render_tool(
    tool_type: &ToolType,
    input: &str,
    output: Option<&String>,
    elapsed: Option<Duration>,
    state: &ToolState,
    width: u16,
    spinner_frame: usize,
) -> Vec<Line<'static>> {
    let output_lines: Vec<String> = output
        .map(|s| s.lines().map(String::from).collect())
        .unwrap_or_default();

    let mut card = ToolCard::new(tool_type.clone(), input)
        .state(state.clone())
        .output(output_lines)
        .frame_index(spinner_frame);

    if let Some(duration) = elapsed {
        card = card.elapsed(duration);
    }

    card.render_to_lines(width)
}

fn render_diff(
    path: &str,
    old_content: &str,
    new_content: &str,
    collapsed: bool,
    width: u16,
) -> Vec<Line<'static>> {
    DiffWidget::new(path, old_content, new_content)
        .collapsed(collapsed)
        .render_to_lines(width)
}

fn render_markdown_with_prefix(text: &str, width: u16) -> Vec<Line<'static>> {
    if is_simple_text(text) {
        return render_plaintext_fallback(text, width);
    }

    let markdown_widget = MarkdownWidget::new(text).indent(0).width(width as usize);

    match markdown_widget.render_to_lines() {
        Ok(lines) if !is_content_lost(text, &lines) => {
            let trimmed = trim_trailing_empty_lines(lines);
            add_prefix_to_lines(trimmed)
        }
        _ => render_plaintext_fallback(text, width),
    }
}

fn is_simple_text(text: &str) -> bool {
    let trimmed = text.trim();

    !trimmed.contains('\n')
        && trimmed.len() < 80
        && !trimmed.contains('*')
        && !trimmed.contains('`')
        && !trimmed.contains('#')
        && !trimmed.contains('[')
}

fn is_content_lost(original: &str, rendered: &[Line<'_>]) -> bool {
    let has_alphanumeric = original.chars().any(|c| c.is_alphanumeric());

    if !has_alphanumeric {
        return false;
    }

    let rendered_has_content = rendered
        .iter()
        .flat_map(|line| line.spans.iter())
        .any(|span| span.content.chars().any(|c| c.is_alphanumeric()));

    !rendered_has_content
}

fn trim_trailing_empty_lines(mut lines: Vec<Line<'static>>) -> Vec<Line<'static>> {
    while lines.last().is_some_and(|l| l.spans.is_empty()) {
        lines.pop();
    }
    lines
}

fn add_prefix_to_lines(lines: Vec<Line<'static>>) -> Vec<Line<'static>> {
    lines
        .into_iter()
        .enumerate()
        .map(|(i, line)| {
            let prefix = if i == 0 { "● " } else { "  " };
            let mut spans = vec![Span::styled(prefix, Theme::off_white())];

            for span in line.spans {
                spans.push(Span::styled(span.content, Theme::off_white()));
            }

            Line::from(spans)
        })
        .collect()
}

fn render_plaintext_fallback(text: &str, width: u16) -> Vec<Line<'static>> {
    let wrapped = textwrap::wrap(text, (width as usize).saturating_sub(4));

    wrapped
        .iter()
        .take(MAX_MESSAGE_LINES)
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
}

fn append_cursor(lines: &mut Vec<Line<'static>>) {
    let cursor = Span::styled("▊", Theme::primary());

    if lines.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("● ", Theme::off_white()),
            cursor,
        ]));
    } else if let Some(last_line) = lines.last_mut() {
        last_line.spans.push(cursor);
    }
}

fn truncate_with_indicator(lines: &mut Vec<Line<'static>>) {
    if lines.len() > MAX_MESSAGE_LINES {
        lines.truncate(MAX_MESSAGE_LINES);
        lines.push(Line::from(Span::styled(
            "  ... (message truncated)",
            Theme::muted(),
        )));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_message_has_prefix() {
        let msg = ChatMessage::User("Hello".to_string());
        let lines = msg.render_to_lines(80, 0);

        assert_eq!(lines.len(), 1);
        let first_span = &lines[0].spans[0];
        assert!(first_span.content.contains('>'));
    }

    #[test]
    fn streaming_has_cursor() {
        let msg = ChatMessage::StreamingAssistant("Hello".to_string());
        let lines = msg.render_to_lines(80, 0);

        let last_line = lines.last().expect("should have lines");
        let has_cursor = last_line.spans.iter().any(|s| s.content.contains('▊'));
        assert!(has_cursor, "streaming message should have cursor");
    }

    #[test]
    fn empty_streaming_shows_cursor() {
        let msg = ChatMessage::StreamingAssistant(String::new());
        let lines = msg.render_to_lines(80, 0);

        assert!(!lines.is_empty());
        let has_cursor = lines[0].spans.iter().any(|s| s.content.contains('▊'));
        assert!(has_cursor);
    }

    #[test]
    fn assistant_truncates_long_messages() {
        let long_text = "Line\n".repeat(100);
        let msg = ChatMessage::Assistant(long_text);
        let lines = msg.render_to_lines(80, 0);

        assert!(lines.len() <= MAX_MESSAGE_LINES + 1);

        let last_line = lines.last().unwrap();
        let has_truncation = last_line
            .spans
            .iter()
            .any(|s| s.content.contains("truncated"));
        assert!(has_truncation);
    }

    #[test]
    fn diff_collapsed_has_fewer_lines() {
        let expanded = ChatMessage::FileDiff {
            path: "/test.rs".into(),
            old_content: "a\nb\nc\n".into(),
            new_content: "a\nmodified\nc\nd\n".into(),
            collapsed: false,
        };

        let collapsed = ChatMessage::FileDiff {
            path: "/test.rs".into(),
            old_content: "a\nb\nc\n".into(),
            new_content: "a\nmodified\nc\nd\n".into(),
            collapsed: true,
        };

        let expanded_lines = expanded.render_to_lines(80, 0);
        let collapsed_lines = collapsed.render_to_lines(80, 0);

        assert!(collapsed_lines.len() < expanded_lines.len());
    }
}
