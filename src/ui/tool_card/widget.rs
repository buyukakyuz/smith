use ratatui::{buffer::Buffer, layout::Rect, widgets::Widget};

use super::card::ToolCard;

const MIN_WIDTH: u16 = 20;
const MIN_HEIGHT: u16 = 3;

impl Widget for ToolCard {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
            return;
        }

        let lines = self.render_to_lines(area.width);

        for (offset, line) in lines.into_iter().take(area.height as usize).enumerate() {
            buf.set_line(area.x, area.y + offset as u16, &line, area.width);
        }
    }
}
