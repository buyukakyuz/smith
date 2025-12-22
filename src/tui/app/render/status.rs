use crate::ui::theme::{Spinners, Theme};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use std::time::Duration;

pub fn render_status(
    frame: &mut Frame,
    area: Rect,
    is_processing: bool,
    elapsed: Option<Duration>,
    spinner_frame: usize,
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
        let line = Line::from(vec![Span::raw(" "), Span::styled(hints, Theme::muted())]);

        let paragraph = Paragraph::new(line);
        frame.render_widget(paragraph, area);
    }
}
