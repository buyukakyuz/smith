use crate::core::types::Usage;
use crate::ui::theme::{Spinners, Theme};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use std::time::Duration;

fn format_tokens(count: u32) -> String {
    if count >= 1_000_000 {
        format!("{:.1}M", f64::from(count) / 1_000_000.0)
    } else if count >= 1_000 {
        format!("{:.1}K", f64::from(count) / 1_000.0)
    } else {
        count.to_string()
    }
}

pub fn render_status(
    frame: &mut Frame,
    area: Rect,
    is_processing: bool,
    elapsed: Option<Duration>,
    spinner_frame: usize,
    last_usage: Option<&Usage>,
    session_usage: &Usage,
) {
    let hints = "/ commands | PgUp/PgDn scroll";
    let hints_width = hints.len() as u16;

    if is_processing {
        let frames = Spinners::BRAILLE;
        let frame_char = frames[spinner_frame % frames.len()];

        let elapsed_text = elapsed
            .map(|d| {
                let secs = d.as_secs();
                if secs > 0 {
                    format!(" {secs}s")
                } else {
                    format!(" {}ms", d.as_millis())
                }
            })
            .unwrap_or_default();

        let status_msg = format!("{frame_char} Processing{elapsed_text}");

        let left_line = Line::from(vec![Span::raw(" "), Span::styled(hints, Theme::muted())]);

        let right_line = Line::from(vec![
            Span::styled(status_msg, Theme::warning()),
            Span::raw(" "),
        ]);

        frame
            .buffer_mut()
            .set_line(area.x, area.y, &left_line, hints_width + 2);

        let status_len = (right_line.width() + 1) as u16;
        let status_x = area.x + area.width.saturating_sub(status_len);
        frame
            .buffer_mut()
            .set_line(status_x, area.y, &right_line, status_len);
    } else {
        let left_line = Line::from(vec![Span::raw(" "), Span::styled(hints, Theme::muted())]);
        frame
            .buffer_mut()
            .set_line(area.x, area.y, &left_line, hints_width + 2);

        let usage_text = if let Some(usage) = last_usage {
            format!(
                "Last: {}in/{}out | Session: {}",
                format_tokens(usage.input_tokens),
                format_tokens(usage.output_tokens),
                format_tokens(session_usage.total())
            )
        } else if session_usage.total() > 0 {
            format!("Session: {} tokens", format_tokens(session_usage.total()))
        } else {
            String::new()
        };

        if !usage_text.is_empty() {
            let right_line = Line::from(vec![
                Span::styled(usage_text, Theme::muted()),
                Span::raw(" "),
            ]);
            let status_len = (right_line.width() + 1) as u16;
            let status_x = area.x + area.width.saturating_sub(status_len);
            frame
                .buffer_mut()
                .set_line(status_x, area.y, &right_line, status_len);
        }
    }
}
