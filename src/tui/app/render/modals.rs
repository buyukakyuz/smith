use crate::permission::types::PermissionRequest;
use crate::ui::theme::Theme;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

pub fn render_permission_modal(
    frame: &mut Frame,
    area: Rect,
    request: &PermissionRequest,
    selected: usize,
    input_mode: bool,
    feedback_input: &str,
) {
    let modal_width = (f32::from(area.width) * 0.6).max(50.0).min(80.0) as u16;
    let modal_height = if input_mode { 14 } else { 12 };

    let modal_x = (area.width.saturating_sub(modal_width)) / 2;
    let modal_y = (area.height.saturating_sub(modal_height)) / 2;

    let modal_area = Rect {
        x: modal_x,
        y: modal_y,
        width: modal_width,
        height: modal_height,
    };

    frame.render_widget(Clear, modal_area);

    let title = format!(" {} ", request.operation_type);
    let target = &request.target;

    let is_dangerous = matches!(
        request.operation_type,
        crate::permission::types::PermissionType::FileDelete
            | crate::permission::types::PermissionType::CommandExecute
            | crate::permission::types::PermissionType::SystemModification
    );

    let border_style = if is_dangerous {
        Theme::warning()
    } else {
        Theme::primary()
    };

    let block = Block::default()
        .title(title)
        .title_style(Theme::primary_bold())
        .borders(Borders::ALL)
        .border_style(border_style)
        .border_set(ratatui::symbols::border::ROUNDED);

    let inner_area = block.inner(modal_area);
    frame.render_widget(block, modal_area);

    let chunks = if input_mode {
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
    };

    let target_display = if target.len() > (inner_area.width as usize - 2) {
        format!(
            "...{}",
            &target[target.len().saturating_sub(inner_area.width as usize - 5)..]
        )
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
            x: chunks[0].x + 1,
            ..chunks[0]
        },
    );

    if let Some(context) = &request.context {
        let context_line = Line::from(Span::styled(context.clone(), Theme::muted()));
        frame.render_widget(
            Paragraph::new(context_line),
            Rect {
                x: chunks[0].x + 1,
                y: chunks[0].y + 1,
                ..chunks[0]
            },
        );
    }

    if input_mode {
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

        let input_display = format!("> {feedback_input}_");
        let input_line = Line::from(Span::styled(input_display, Theme::primary()));
        frame.render_widget(
            Paragraph::new(input_line),
            Rect {
                x: chunks[3].x + 1,
                ..chunks[3]
            },
        );

        let hint = "Enter: submit • Esc: cancel";
        let hint_line = Line::from(Span::styled(hint, Theme::muted()));
        frame.render_widget(
            Paragraph::new(hint_line).alignment(Alignment::Center),
            chunks[5],
        );
    } else {
        let options = [
            ("1", "Allow once", "Allow this single operation"),
            ("2", "Allow for session", "Allow all similar operations"),
            ("3", "Deny", "Tell the model to try something else"),
        ];

        for (i, (key, label, desc)) in options.iter().enumerate() {
            let is_selected = i == selected;
            let y = chunks[2].y + i as u16;

            let prefix = if is_selected { "▸ " } else { "  " };
            let key_style = if is_selected {
                Theme::primary_bold()
            } else {
                Theme::muted()
            };
            let label_style = if is_selected {
                Theme::primary()
            } else {
                ratatui::style::Style::default()
            };
            let desc_style = Theme::muted();

            let line = Line::from(vec![
                Span::styled(prefix, key_style),
                Span::styled(format!("[{key}] "), key_style),
                Span::styled(label.to_string(), label_style),
                Span::styled(format!(" - {desc}"), desc_style),
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

        let hint = "↑/↓: navigate • Enter: confirm • Esc: cancel • y: allow • 3/n: deny";
        let hint_line = Line::from(Span::styled(hint, Theme::muted()));
        frame.render_widget(
            Paragraph::new(hint_line).alignment(Alignment::Center),
            chunks[3],
        );
    }
}

pub fn render_model_picker_modal(
    frame: &mut Frame,
    area: Rect,
    models: &[(
        crate::config::ProviderType,
        Vec<crate::tui::state::PickerModel>,
    )],
    selected: usize,
    total_count: usize,
) {
    let modal_width = (f32::from(area.width) * 0.5).max(40.0).min(60.0) as u16;
    let content_height = total_count as u16 + models.len() as u16 + 2;
    let modal_height = content_height.min(area.height.saturating_sub(4)) + 4;

    let modal_x = (area.width.saturating_sub(modal_width)) / 2;
    let modal_y = (area.height.saturating_sub(modal_height)) / 2;

    let modal_area = Rect {
        x: modal_x,
        y: modal_y,
        width: modal_width,
        height: modal_height,
    };

    frame.render_widget(Clear, modal_area);

    let block = Block::default()
        .title(" Select Model ")
        .title_style(Theme::primary_bold())
        .borders(Borders::ALL)
        .border_style(Theme::primary())
        .border_set(ratatui::symbols::border::ROUNDED);

    let inner_area = block.inner(modal_area);
    frame.render_widget(block, modal_area);

    let chunks = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(inner_area);

    let mut y = chunks[0].y;
    let mut flat_idx = 0;

    for (provider, provider_models) in models {
        let provider_line = Line::from(Span::styled(
            format!("  {}", provider.display_name()),
            Theme::secondary(),
        ));
        frame.render_widget(
            Paragraph::new(provider_line),
            Rect {
                x: chunks[0].x,
                y,
                width: chunks[0].width,
                height: 1,
            },
        );
        y += 1;

        for model in provider_models {
            if y >= chunks[0].y + chunks[0].height {
                break;
            }

            let is_selected = flat_idx == selected;
            let prefix = if is_selected { "  ▸ " } else { "    " };

            let style = if is_selected {
                Theme::primary_bold()
            } else {
                ratatui::style::Style::default()
            };

            let line = Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(&model.name, style),
            ]);

            frame.render_widget(
                Paragraph::new(line),
                Rect {
                    x: chunks[0].x,
                    y,
                    width: chunks[0].width,
                    height: 1,
                },
            );

            y += 1;
            flat_idx += 1;
        }

        if y < chunks[0].y + chunks[0].height {
            y += 1;
        }
    }

    let hint = "↑/↓: navigate • Enter: select • Esc: cancel";
    let hint_line = Line::from(Span::styled(hint, Theme::muted()));
    frame.render_widget(
        Paragraph::new(hint_line).alignment(Alignment::Center),
        chunks[1],
    );
}
