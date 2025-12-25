mod line_builder;
mod parser;
mod types;

use ratatui::{buffer::Buffer, layout::Rect, widgets::Widget};

use line_builder::LineBuilder;
use parser::ParsedDiff;

pub use types::ChangeType;

#[derive(Debug, Clone)]
pub struct DiffWidget {
    path: String,
    change_type: ChangeType,
    additions: usize,
    deletions: usize,
    lines: Vec<types::DiffLine>,
    show_line_numbers: bool,
    collapsed: bool,
}

impl DiffWidget {
    #[must_use]
    pub fn new(path: impl Into<String>, old_content: &str, new_content: &str) -> Self {
        let parsed = ParsedDiff::new(old_content, new_content);

        Self {
            path: path.into(),
            change_type: parsed.change_type,
            additions: parsed.additions,
            deletions: parsed.deletions,
            lines: parsed.lines,
            show_line_numbers: true,
            collapsed: false,
        }
    }

    #[must_use]
    pub const fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }

    #[must_use]
    pub const fn show_line_numbers(mut self, show: bool) -> Self {
        self.show_line_numbers = show;
        self
    }

    #[must_use]
    pub const fn change_type(&self) -> ChangeType {
        self.change_type
    }

    #[must_use]
    pub const fn additions(&self) -> usize {
        self.additions
    }

    #[must_use]
    pub const fn deletions(&self) -> usize {
        self.deletions
    }

    #[must_use]
    pub fn render_to_lines(&self, width: u16) -> Vec<ratatui::text::Line<'static>> {
        if width < 20 {
            return vec![];
        }

        self.line_builder().build(width, self.collapsed)
    }

    fn line_builder(&self) -> LineBuilder<'_> {
        LineBuilder::new(
            &self.path,
            self.change_type,
            self.additions,
            self.deletions,
            &self.lines,
            self.show_line_numbers,
        )
    }
}

impl Widget for DiffWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 20 || area.height < 3 {
            return;
        }

        let lines = self.render_to_lines(area.width);

        for (i, line) in lines.into_iter().enumerate() {
            let y = area.y + i as u16;
            if y >= area.y + area.height {
                break;
            }
            buf.set_line(area.x, y, &line, area.width);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_diff_for_new_file() {
        let diff = DiffWidget::new("src/new.rs", "", "fn main() {}\n");
        assert_eq!(diff.change_type(), ChangeType::Create);
        assert_eq!(diff.additions(), 1);
        assert_eq!(diff.deletions(), 0);
    }

    #[test]
    fn creates_diff_for_deletion() {
        let diff = DiffWidget::new("src/old.rs", "fn old() {}\n", "");
        assert_eq!(diff.change_type(), ChangeType::Delete);
        assert_eq!(diff.additions(), 0);
        assert_eq!(diff.deletions(), 1);
    }

    #[test]
    fn creates_diff_for_update() {
        let diff = DiffWidget::new("src/file.rs", "old line\n", "new line\n");
        assert_eq!(diff.change_type(), ChangeType::Update);
        assert_eq!(diff.additions(), 1);
        assert_eq!(diff.deletions(), 1);
    }

    #[test]
    fn collapsed_renders_header_only() {
        let diff = DiffWidget::new("src/file.rs", "old\n", "new\n").collapsed(true);
        let lines = diff.render_to_lines(80);
        assert_eq!(lines.len(), 2);
    }
}
