use crate::permission::types::{PermissionRequest, PermissionType};
use crate::ui::theme::Theme;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};

use super::{calc_centered_modal, create_modal_block, render_hint};

pub fn render_permission_modal(
    frame: &mut Frame,
    area: Rect,
    request: &PermissionRequest,
    selected: usize,
    input_mode: bool,
    feedback_input: &str,
) {
    let modal_area = calc_centered_modal(area, 0.6, 50.0, 80.0, if input_mode { 14 } else { 12 });
    frame.render_widget(Clear, modal_area);

    let border_style = if is_dangerous_operation(request) {
        Theme::warning()
    } else {
        Theme::primary()
    };
    let block = create_modal_block(&request.operation_type.to_string(), border_style);
    let inner_area = block.inner(modal_area);
    frame.render_widget(block, modal_area);

    let chunks = create_layout(inner_area, input_mode);
    render_target_section(frame, chunks[0], request, inner_area.width);

    if input_mode {
        render_input_mode(frame, &chunks, feedback_input);
    } else {
        render_options_mode(frame, &chunks, selected);
    }
}

const fn is_dangerous_operation(request: &PermissionRequest) -> bool {
    matches!(
        request.operation_type,
        PermissionType::FileDelete
            | PermissionType::CommandExecute
            | PermissionType::SystemModification
    )
}

fn create_layout(inner_area: Rect, input_mode: bool) -> std::rc::Rc<[Rect]> {
    if input_mode {
        Layout::vertical([
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(inner_area)
    } else {
        Layout::vertical([
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Min(5),
            Constraint::Length(1),
        ])
        .split(inner_area)
    }
}

fn render_target_section(
    frame: &mut Frame,
    chunk: Rect,
    request: &PermissionRequest,
    max_width: u16,
) {
    let target = &request.target;
    let max_len = max_width as usize - 2;
    let target_display = if target.len() > max_len {
        format!("...{}", &target[target.len().saturating_sub(max_len - 3)..])
    } else {
        target.clone()
    };

    let target_line = Line::from(vec![
        Span::styled("Target: ", Theme::muted()),
        Span::styled(target_display, Theme::secondary()),
    ]);
    frame.render_widget(
        Paragraph::new(target_line).alignment(Alignment::Left),
        Rect {
            x: chunk.x + 1,
            ..chunk
        },
    );

    if let Some(context) = &request.context {
        let context_line = Line::from(Span::styled(context.clone(), Theme::muted()));
        frame.render_widget(
            Paragraph::new(context_line),
            Rect {
                x: chunk.x + 1,
                y: chunk.y + 1,
                ..chunk
            },
        );
    }
}

fn render_input_mode(frame: &mut Frame, chunks: &[Rect], feedback_input: &str) {
    let prompt_line = Line::from(Span::styled(
        "Tell the model what to do instead:",
        Theme::secondary(),
    ));
    frame.render_widget(
        Paragraph::new(prompt_line),
        Rect {
            x: chunks[2].x + 1,
            ..chunks[2]
        },
    );

    let input_line = Line::from(Span::styled(
        format!("> {feedback_input}_"),
        Theme::primary(),
    ));
    frame.render_widget(
        Paragraph::new(input_line),
        Rect {
            x: chunks[3].x + 1,
            ..chunks[3]
        },
    );

    render_hint(frame, chunks[5], "Enter: submit • Esc: cancel");
}

fn render_options_mode(frame: &mut Frame, chunks: &[Rect], selected: usize) {
    const OPTIONS: [(&str, &str, &str); 3] = [
        ("1", "Allow once", "Allow this single operation"),
        ("2", "Allow for session", "Allow all similar operations"),
        ("3", "Deny", "Tell the model to try something else"),
    ];

    for (i, (key, label, desc)) in OPTIONS.iter().enumerate() {
        let is_selected = i == selected;
        let y = chunks[2].y + i as u16;

        let (prefix, key_style, label_style) = if is_selected {
            ("▸ ", Theme::primary_bold(), Theme::primary())
        } else {
            ("  ", Theme::muted(), ratatui::style::Style::default())
        };

        let line = Line::from(vec![
            Span::styled(prefix, key_style),
            Span::styled(format!("[{key}] "), key_style),
            Span::styled(*label, label_style),
            Span::styled(format!(" - {desc}"), Theme::muted()),
        ]);

        frame.render_widget(
            Paragraph::new(line),
            Rect {
                x: chunks[2].x + 1,
                y,
                width: chunks[2].width.saturating_sub(2),
                height: 1,
            },
        );
    }

    render_hint(
        frame,
        chunks[3],
        "↑/↓: navigate • Enter: confirm • Esc: cancel • y: allow • 3/n: deny",
    );
}
