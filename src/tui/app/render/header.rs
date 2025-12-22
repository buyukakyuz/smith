use crate::ui::theme::{BoxChars, Theme};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

pub fn render_header(frame: &mut Frame, area: Rect, provider_name: &str, model_name: &str) {
    let title = format!("Smith v{}", env!("CARGO_PKG_VERSION"));
    let subtitle = format!("Provider: {provider_name} | Model: {model_name}");

    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Theme::border())
        .border_set(ratatui::symbols::border::Set {
            bottom_left: BoxChars::ROUND_BOTTOM_LEFT,
            bottom_right: BoxChars::ROUND_BOTTOM_RIGHT,
            ..ratatui::symbols::border::ROUNDED
        });

    let lines = vec![
        Line::from(vec![
            Span::raw("  "),
            Span::styled(title, Theme::primary_bold()),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(subtitle, Theme::muted()),
        ]),
    ];

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}
