use ratatui::{
    style::Modifier,
    text::{Line, Span},
};
use textwrap::wrap;

use super::parser::{Hunk, extract_hunks};
use super::types::{ChangeType, DiffLine};
use crate::ui::theme::{BoxChars, Theme};

pub struct LineBuilder<'a> {
    path: &'a str,
    change_type: ChangeType,
    additions: usize,
    deletions: usize,
    lines: &'a [DiffLine],
    show_line_numbers: bool,
}

impl<'a> LineBuilder<'a> {
    pub const fn new(
        path: &'a str,
        change_type: ChangeType,
        additions: usize,
        deletions: usize,
        lines: &'a [DiffLine],
        show_line_numbers: bool,
    ) -> Self {
        Self {
            path,
            change_type,
            additions,
            deletions,
            lines,
            show_line_numbers,
        }
    }

    pub fn build(&self, width: u16, collapsed: bool) -> Vec<Line<'static>> {
        let mut output = Vec::new();

        output.push(self.build_header());
        output.push(self.build_summary());

        if collapsed {
            return output;
        }

        output.push(Line::from(""));

        let content_width = self.content_width(width as usize);
        output.extend(self.build_hunks(content_width));

        output
    }

    fn build_header(&self) -> Line<'static> {
        let text = format!(
            " {} {} ({})",
            self.change_type.icon(),
            self.change_type.label(),
            self.path
        );

        Line::from(Span::styled(
            text,
            self.change_type.style().add_modifier(Modifier::BOLD),
        ))
    }

    fn build_summary(&self) -> Line<'static> {
        let summary = match (self.additions, self.deletions, self.change_type) {
            (0, 0, _) => "No changes".to_string(),
            (a, _, ChangeType::Create) => format!("Added {a} lines"),
            (_, d, ChangeType::Delete) => format!("Removed {d} lines"),
            (a, d, _) => format!("+{a} -{d} lines"),
        };

        Line::from(vec![
            Span::styled("  ", Theme::muted()),
            Span::styled(BoxChars::ELLIPSIS, Theme::muted()),
            Span::styled(format!(" {summary}"), Theme::muted()),
        ])
    }

    fn build_hunks(&self, content_width: usize) -> Vec<Line<'static>> {
        let hunks = extract_hunks(self.lines);
        let mut output = Vec::new();

        for (idx, hunk) in hunks.iter().enumerate() {
            if idx > 0 {
                output.push(self.build_separator(content_width));
            }
            output.extend(self.build_hunk_lines(hunk, content_width));
        }

        output
    }

    fn build_separator(&self, width: usize) -> Line<'static> {
        Line::from(Span::styled(
            format!(
                "  {}",
                BoxChars::DIVIDER_LIGHT.repeat(width.saturating_sub(2))
            ),
            Theme::muted(),
        ))
    }

    fn build_hunk_lines(&self, hunk: &Hunk<'_>, content_width: usize) -> Vec<Line<'static>> {
        let mut output = Vec::new();

        for line in hunk {
            let content = line.content.trim_end_matches('\n');
            let wrapped = wrap(content, content_width);

            for (i, wrapped_line) in wrapped.iter().enumerate().take(3) {
                output.push(self.build_diff_line(line, &wrapped_line, i == 0));
            }
        }

        output
    }

    fn build_diff_line(
        &self,
        line: &DiffLine,
        content: &str,
        show_line_num: bool,
    ) -> Line<'static> {
        let mut spans = vec![Span::raw("  ")];

        if self.show_line_numbers {
            let num_str = if show_line_num {
                format!("{:>6} ", line.line_num)
            } else {
                "       ".to_string()
            };
            spans.push(Span::styled(num_str, Theme::muted()));
        }

        let style = line.tag.style();
        spans.push(Span::styled(line.tag.indicator(), style));
        spans.push(Span::raw(" "));
        spans.push(Span::styled(content.to_string(), style));

        Line::from(spans)
    }

    fn content_width(&self, total_width: usize) -> usize {
        let padding = if self.show_line_numbers { 10 } else { 4 };
        total_width.saturating_sub(padding)
    }
}
