use ratatui::layout::{Constraint, Direction, Layout, Rect};

pub struct LayoutAreas {
    pub header: Rect,
    pub chat: Rect,
    pub input: Rect,
    pub status: Rect,
}

#[must_use]
pub fn calculate_layout(area: Rect) -> LayoutAreas {
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    let middle_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(3)])
        .split(main_chunks[1]);

    LayoutAreas {
        header: main_chunks[0],
        chat: middle_chunks[0],
        input: middle_chunks[1],
        status: main_chunks[2],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_calculation() {
        let area = Rect::new(0, 0, 100, 40);
        let layout = calculate_layout(area);

        assert_eq!(layout.header.height, 3);
        assert_eq!(layout.status.height, 1);
        assert_eq!(layout.input.height, 3);

        let expected_chat_height = 40 - 3 - 3 - 1;
        assert_eq!(layout.chat.height, expected_chat_height);
    }

    #[test]
    fn test_layout_minimum_chat_height() {
        let area = Rect::new(0, 0, 80, 10);
        let layout = calculate_layout(area);

        assert!(layout.chat.height > 0);
    }
}
