use crate::ui::theme::{BoxChars, Theme};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Widget,
};
use similar::{ChangeTag, TextDiff};
use textwrap::wrap;

const CONTEXT_LINES: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeType {
    Create,
    Update,
    Delete,
}

#[derive(Debug, Clone, PartialEq)]
struct DiffLine {
    line_num: usize,
    change_type: char,
    content: String,
}

#[derive(Debug, Clone)]
pub struct DiffWidget {
    path: String,
    change_type: ChangeType,
    additions: usize,
    deletions: usize,
    lines: Vec<DiffLine>,
    show_line_numbers: bool,
    collapsed: bool,
}

impl DiffWidget {
    #[must_use]
    pub fn new(path: impl Into<String>, old_content: &str, new_content: &str) -> Self {
        let diff = TextDiff::from_lines(old_content, new_content);

        let mut additions = 0;
        let mut deletions = 0;
        let mut lines = Vec::new();
        let mut old_line_num = 1;
        let mut new_line_num = 1;

        for change in diff.iter_all_changes() {
            let (line_num, change_char) = match change.tag() {
                ChangeTag::Delete => {
                    deletions += 1;
                    let num = old_line_num;
                    old_line_num += 1;
                    (num, '-')
                }
                ChangeTag::Insert => {
                    additions += 1;
                    let num = new_line_num;
                    new_line_num += 1;
                    (num, '+')
                }
                ChangeTag::Equal => {
                    let num = new_line_num;
                    old_line_num += 1;
                    new_line_num += 1;
                    (num, ' ')
                }
            };

            lines.push(DiffLine {
                line_num,
                change_type: change_char,
                content: change.to_string(),
            });
        }

        let change_type = if old_content.is_empty() {
            ChangeType::Create
        } else if new_content.is_empty() {
            ChangeType::Delete
        } else {
            ChangeType::Update
        };

        Self {
            path: path.into(),
            change_type,
            additions,
            deletions,
            lines,
            show_line_numbers: true,
            collapsed: false,
        }
    }

    #[must_use]
    pub const fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }

    const fn icon(&self) -> &'static str {
        match self.change_type {
            ChangeType::Create => "+",
            ChangeType::Update => "~",
            ChangeType::Delete => "-",
        }
    }

    const fn label(&self) -> &'static str {
        match self.change_type {
            ChangeType::Create => "Create",
            ChangeType::Update => "Update",
            ChangeType::Delete => "Delete",
        }
    }

    const fn change_style(&self) -> Style {
        match self.change_type {
            ChangeType::Create => Theme::success(),
            ChangeType::Update => Theme::primary(),
            ChangeType::Delete => Theme::error(),
        }
    }

    fn get_hunks(&self) -> Vec<Vec<&DiffLine>> {
        let mut hunks = Vec::new();
        let mut current_hunk = Vec::new();
        let mut last_change_idx = None;

        for (idx, line) in self.lines.iter().enumerate() {
            let is_change = line.change_type != ' ';

            if is_change {
                let start_context = last_change_idx.map_or_else(
                    || idx.saturating_sub(CONTEXT_LINES),
                    |last_idx| idx.saturating_sub(CONTEXT_LINES).max(last_idx + 1),
                );

                for i in start_context..idx {
                    if i < self.lines.len() && !current_hunk.contains(&&self.lines[i]) {
                        current_hunk.push(&self.lines[i]);
                    }
                }

                current_hunk.push(line);
                last_change_idx = Some(idx);
            } else if let Some(last_idx) = last_change_idx {
                if idx - last_idx <= CONTEXT_LINES {
                    current_hunk.push(line);
                } else if idx - last_idx == CONTEXT_LINES + 1 {
                    current_hunk.push(line);
                    hunks.push(current_hunk);
                    current_hunk = Vec::new();
                    last_change_idx = None;
                }
            }
        }

        if !current_hunk.is_empty() {
            hunks.push(current_hunk);
        }

        hunks
    }

    fn build_header_line(&self) -> Line<'static> {
        let icon = self.icon();
        let label = self.label();
        let path_display = format!(" {} {} ({})", icon, label, self.path);

        Line::from(vec![Span::styled(
            path_display,
            self.change_style().add_modifier(Modifier::BOLD),
        )])
    }

    fn build_summary_line(&self) -> Line<'static> {
        let summary = if self.additions == 0 && self.deletions == 0 {
            "No changes".to_string()
        } else if self.change_type == ChangeType::Create {
            format!("Added {} lines", self.additions)
        } else if self.change_type == ChangeType::Delete {
            format!("Removed {} lines", self.deletions)
        } else {
            format!("+{} -{} lines", self.additions, self.deletions)
        };

        Line::from(vec![
            Span::styled("  ", Theme::muted()),
            Span::styled(BoxChars::ELLIPSIS, Theme::muted()),
            Span::styled(format!(" {summary}"), Theme::muted()),
        ])
    }

    fn build_hunk_lines(&self, content_width: usize) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        let hunks = self.get_hunks();

        for (hunk_idx, hunk) in hunks.iter().enumerate() {
            if hunk_idx > 0 {
                let separator = Line::from(Span::styled(
                    format!(
                        "  {}",
                        BoxChars::DIVIDER_LIGHT.repeat(content_width.saturating_sub(2))
                    ),
                    Theme::muted(),
                ));
                lines.push(separator);
            }

            for line in hunk {
                let content = line.content.trim_end_matches('\n');

                let wrapped = wrap(content, content_width);
                for (i, wrapped_line) in wrapped.iter().enumerate().take(3) {
                    let mut spans = vec![Span::raw("  ")];

                    if self.show_line_numbers && i == 0 {
                        let line_num_str = format!("{:>6} ", line.line_num);
                        spans.push(Span::styled(line_num_str, Theme::muted()));
                    } else if self.show_line_numbers {
                        spans.push(Span::raw("       "));
                    }

                    let (indicator, style) = match line.change_type {
                        '-' => ("-", Theme::error()),
                        '+' => ("+", Theme::success()),
                        _ => (" ", Style::default()),
                    };

                    spans.push(Span::styled(indicator, style));
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(wrapped_line.to_string(), style));

                    lines.push(Line::from(spans));
                }
            }
        }

        lines
    }

    pub fn render_to_lines(&self, width: u16) -> Vec<Line<'static>> {
        if width < 20 {
            return vec![];
        }

        let mut lines = Vec::new();

        lines.push(self.build_header_line());

        lines.push(self.build_summary_line());

        if self.collapsed {
            return lines;
        }

        lines.push(Line::from(""));

        let content_width =
            (width as usize).saturating_sub(if self.show_line_numbers { 10 } else { 4 });
        lines.extend(self.build_hunk_lines(content_width));

        lines
    }
}

impl Widget for DiffWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 20 || area.height < 3 {
            return;
        }

        let width = area.width as usize;
        let mut y = area.y;

        let icon = self.icon();
        let label = self.label();
        let path_display = format!(" {} {} ({})", icon, label, self.path);

        let header_line = Line::from(vec![Span::styled(
            path_display,
            self.change_style().add_modifier(Modifier::BOLD),
        )]);

        buf.set_line(area.x, y, &header_line, area.width);
        y += 1;

        let summary = if self.additions == 0 && self.deletions == 0 {
            "No changes".to_string()
        } else if self.change_type == ChangeType::Create {
            format!("Added {} lines", self.additions)
        } else if self.change_type == ChangeType::Delete {
            format!("Removed {} lines", self.deletions)
        } else {
            format!("+{} -{} lines", self.additions, self.deletions)
        };

        let summary_line = Line::from(vec![
            Span::styled("  ", Theme::muted()),
            Span::styled(BoxChars::ELLIPSIS, Theme::muted()),
            Span::styled(format!(" {summary}"), Theme::muted()),
        ]);

        buf.set_line(area.x, y, &summary_line, area.width);
        y += 1;

        if self.collapsed {
            return;
        }

        buf.set_line(area.x, y, &Line::from(""), area.width);
        y += 1;

        let hunks = self.get_hunks();
        let content_width = width.saturating_sub(if self.show_line_numbers { 10 } else { 4 });

        for (hunk_idx, hunk) in hunks.iter().enumerate() {
            if hunk_idx > 0 && y < area.y + area.height {
                let separator = Line::from(Span::styled(
                    format!(
                        "  {}",
                        BoxChars::DIVIDER_LIGHT.repeat(width.saturating_sub(2))
                    ),
                    Theme::muted(),
                ));
                buf.set_line(area.x, y, &separator, area.width);
                y += 1;
            }

            for line in hunk {
                if y >= area.y + area.height {
                    break;
                }

                let content = line.content.trim_end_matches('\n');

                let wrapped = wrap(content, content_width);
                for (i, wrapped_line) in wrapped.iter().enumerate().take(3) {
                    if y >= area.y + area.height {
                        break;
                    }

                    let mut spans = vec![Span::raw("  ")];

                    if self.show_line_numbers && i == 0 {
                        let line_num_str = format!("{:>6} ", line.line_num);
                        spans.push(Span::styled(line_num_str, Theme::muted()));
                    } else if self.show_line_numbers {
                        spans.push(Span::raw("       "));
                    }

                    let (indicator, style) = match line.change_type {
                        '-' => ("-", Theme::error()),
                        '+' => ("+", Theme::success()),
                        _ => (" ", Style::default()),
                    };

                    spans.push(Span::styled(indicator, style));
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(wrapped_line.to_string(), style));

                    let diff_line = Line::from(spans);
                    buf.set_line(area.x, y, &diff_line, area.width);
                    y += 1;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_widget_create() {
        let diff = DiffWidget::new("src/new.rs", "", "fn main() {}\n");
        assert_eq!(diff.change_type, ChangeType::Create);
        assert_eq!(diff.additions, 1);
        assert_eq!(diff.deletions, 0);
    }

    #[test]
    fn test_diff_widget_delete() {
        let diff = DiffWidget::new("src/old.rs", "fn old() {}\n", "");
        assert_eq!(diff.change_type, ChangeType::Delete);
        assert_eq!(diff.additions, 0);
        assert_eq!(diff.deletions, 1);
    }

    #[test]
    fn test_diff_widget_update() {
        let diff = DiffWidget::new("src/file.rs", "old line\n", "new line\n");
        assert_eq!(diff.change_type, ChangeType::Update);
        assert_eq!(diff.additions, 1);
        assert_eq!(diff.deletions, 1);
    }

    #[test]
    fn test_diff_widget_no_changes() {
        let content = "same\n";
        let diff = DiffWidget::new("src/same.rs", content, content);
        assert_eq!(diff.additions, 0);
        assert_eq!(diff.deletions, 0);
    }
}
