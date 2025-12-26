use crate::config::ProviderType;
use crate::tui::state::PickerModel;
use crate::ui::theme::Theme;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};

use super::{calc_centered_modal, create_modal_block, render_hint};

pub fn render_model_picker_modal(
    frame: &mut Frame,
    area: Rect,
    models: &[(ProviderType, Vec<PickerModel>)],
    selected: usize,
    total_count: usize,
) {
    let content_height = total_count as u16 + models.len() as u16 + 2;
    let modal_height = content_height.min(area.height.saturating_sub(4)) + 4;
    let modal_area = calc_centered_modal(area, 0.5, 40.0, 60.0, modal_height);

    frame.render_widget(Clear, modal_area);

    let block = create_modal_block("Select Model", Theme::primary());
    let inner_area = block.inner(modal_area);
    frame.render_widget(block, modal_area);

    let chunks = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(inner_area);

    render_model_list(frame, chunks[0], models, selected);
    render_hint(
        frame,
        chunks[1],
        "↑/↓: navigate • Enter: select • Esc: cancel",
    );
}

fn render_model_list(
    frame: &mut Frame,
    chunk: Rect,
    models: &[(ProviderType, Vec<PickerModel>)],
    selected: usize,
) {
    let mut y = chunk.y;
    let mut flat_idx = 0;

    for (provider, provider_models) in models {
        let provider_line = Line::from(Span::styled(
            format!("  {}", provider.display_name()),
            Theme::secondary(),
        ));
        frame.render_widget(
            Paragraph::new(provider_line),
            Rect {
                x: chunk.x,
                y,
                width: chunk.width,
                height: 1,
            },
        );
        y += 1;

        for model in provider_models {
            if y >= chunk.y + chunk.height {
                break;
            }

            let is_selected = flat_idx == selected;
            let (prefix, style) = if is_selected {
                ("  ▸ ", Theme::primary_bold())
            } else {
                ("    ", ratatui::style::Style::default())
            };

            let line = Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(&model.name, style),
            ]);

            frame.render_widget(
                Paragraph::new(line),
                Rect {
                    x: chunk.x,
                    y,
                    width: chunk.width,
                    height: 1,
                },
            );

            y += 1;
            flat_idx += 1;
        }

        if y < chunk.y + chunk.height {
            y += 1;
        }
    }
}
