use crate::core::types::Usage;
use crate::ui::theme::{Spinners, Theme};
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use std::time::Duration;

const HINTS: &str = "/ commands | PgUp/PgDn scroll";

fn format_tokens(count: u32) -> String {
    if count >= 1_000_000 {
        format!("{:.1}M", f64::from(count) / 1_000_000.0)
    } else if count >= 1_000 {
        format!("{:.1}K", f64::from(count) / 1_000.0)
    } else {
        count.to_string()
    }
}

fn render_hints(buf: &mut Buffer, area: Rect) {
    let line = Line::from(vec![Span::raw(" "), Span::styled(HINTS, Theme::muted())]);
    buf.set_line(area.x, area.y, &line, HINTS.len() as u16 + 2);
}

fn render_right_text(buf: &mut Buffer, area: Rect, line: &Line) {
    let width = (line.width() + 1) as u16;
    let x = area.x + area.width.saturating_sub(width);
    buf.set_line(x, area.y, line, width);
}

fn format_elapsed(elapsed: Option<Duration>) -> String {
    elapsed
        .map(|d| {
            let secs = d.as_secs();
            if secs > 0 {
                format!(" {secs}s")
            } else {
                format!(" {}ms", d.as_millis())
            }
        })
        .unwrap_or_default()
}

fn format_usage(last_usage: Option<&Usage>, session_usage: Usage) -> Option<String> {
    match last_usage {
        Some(usage) if usage.input_tokens > 0 || usage.output_tokens > 0 => Some(format!(
            "Last: {}in/{}out | Session: {}",
            format_tokens(usage.input_tokens),
            format_tokens(usage.output_tokens),
            format_tokens(session_usage.total())
        )),
        _ if session_usage.total() > 0 => Some(format!(
            "Session: {} tokens",
            format_tokens(session_usage.total())
        )),
        _ => None,
    }
}

pub fn render_status(
    frame: &mut Frame,
    area: Rect,
    is_processing: bool,
    elapsed: Option<Duration>,
    spinner_frame: usize,
    last_usage: Option<&Usage>,
    session_usage: Usage,
) {
    let buf = frame.buffer_mut();
    render_hints(buf, area);

    if is_processing {
        let spinner = Spinners::BRAILLE[spinner_frame % Spinners::BRAILLE.len()];
        let status = format!("{spinner} Processing{}", format_elapsed(elapsed));
        let line = Line::from(vec![Span::styled(status, Theme::warning()), Span::raw(" ")]);
        render_right_text(buf, area, &line);
    } else if let Some(usage) = format_usage(last_usage, session_usage) {
        let line = Line::from(vec![Span::styled(usage, Theme::muted()), Span::raw(" ")]);
        render_right_text(buf, area, &line);
    }
}
