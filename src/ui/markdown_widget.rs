#![allow(clippy::too_many_lines)]
use crate::ui::theme::{BoxChars, Theme};
use markdown::mdast::{Code, Heading, List, ListItem, Node, Paragraph};
use markdown::{ParseOptions, to_mdast};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Widget,
};
use textwrap::wrap;

#[derive(Debug, Clone)]
pub struct MarkdownWidget {
    content: String,
    indent_level: usize,
    width: Option<usize>,
}

impl MarkdownWidget {
    #[must_use]
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            indent_level: 0,
            width: None,
        }
    }

    #[must_use]
    pub const fn indent(mut self, level: usize) -> Self {
        self.indent_level = level;
        self
    }

    #[must_use]
    pub const fn width(mut self, width: usize) -> Self {
        self.width = Some(width);
        self
    }

    pub fn render_to_lines(&self) -> Result<Vec<Line<'static>>, Box<dyn std::error::Error>> {
        let ast = to_mdast(&self.content, &ParseOptions::default()).map_err(|e| {
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e.to_string(),
            )) as Box<dyn std::error::Error>
        })?;

        let mut lines = Vec::new();
        self.render_node(&ast, &mut lines, self.indent_level)?;

        Ok(lines)
    }

    fn render_node(
        &self,
        node: &Node,
        lines: &mut Vec<Line<'static>>,
        indent_level: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match node {
            Node::Root(root) => {
                for child in &root.children {
                    self.render_node(child, lines, indent_level)?;
                }
            }

            Node::Heading(heading) => {
                Self::render_heading(heading, lines, indent_level);
                lines.push(Line::from(""));
            }

            Node::Paragraph(para) => {
                self.render_paragraph(para, lines, indent_level)?;
                lines.push(Line::from(""));
            }

            Node::List(list) => {
                self.render_list(list, lines, indent_level)?;
                lines.push(Line::from(""));
            }

            Node::ListItem(item) => {
                self.render_list_item(item, lines, indent_level)?;
            }

            Node::Code(code) => {
                Self::render_code_block(code, lines, indent_level);
                lines.push(Line::from(""));
            }

            Node::Break(_) | Node::ThematicBreak(_) => {
                lines.push(Line::from(""));
            }

            Node::Blockquote(quote) => {
                for child in &quote.children {
                    self.render_node(child, lines, indent_level + 1)?;
                }
            }

            _ => {}
        }

        Ok(())
    }

    fn render_heading(heading: &Heading, lines: &mut Vec<Line<'static>>, indent_level: usize) {
        let indent = "  ".repeat(indent_level);

        let mut text = String::new();
        for child in &heading.children {
            Self::collect_text(child, &mut text);
        }

        let mut spans = vec![Span::raw(indent)];
        spans.push(Span::styled(
            text,
            Theme::primary().add_modifier(Modifier::BOLD),
        ));

        lines.push(Line::from(spans));
    }

    fn render_paragraph(
        &self,
        para: &Paragraph,
        lines: &mut Vec<Line<'static>>,
        indent_level: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let indent = "  ".repeat(indent_level);
        let indent_width = indent.len();

        let mut all_spans: Vec<Span<'static>> = Vec::new();
        for child in &para.children {
            Self::render_inline_node(child, &mut all_spans)?;
        }

        let full_text: String = all_spans.iter().map(|s| s.content.as_ref()).collect();

        let text_lines: Vec<&str> = full_text.lines().collect();

        let available_width = self.width.unwrap_or(80).saturating_sub(indent_width);
        if available_width == 0 {
            return Ok(());
        }

        for text_line in text_lines {
            if text_line.len() <= available_width || self.width.is_none() {
                let line_spans = vec![
                    Span::raw(indent.clone()),
                    Span::styled(text_line.to_string(), Style::default()),
                ];
                lines.push(Line::from(line_spans));
            } else {
                let wrapped = wrap(text_line, available_width);
                for line_text in &wrapped {
                    let line_spans = vec![
                        Span::raw(indent.clone()),
                        Span::styled(line_text.to_string(), Style::default()),
                    ];
                    lines.push(Line::from(line_spans));
                }
            }
        }

        Ok(())
    }

    fn render_inline_node(
        node: &Node,
        spans: &mut Vec<Span<'static>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match node {
            Node::Text(text) => {
                spans.push(Span::raw(text.value.clone()));
            }

            Node::Strong(strong) => {
                let mut text = String::new();
                for child in &strong.children {
                    Self::collect_text(child, &mut text);
                }
                spans.push(Span::styled(text, Modifier::BOLD));
            }

            Node::Emphasis(emphasis) => {
                for child in &emphasis.children {
                    Self::render_inline_node(child, spans)?;
                }
            }

            Node::InlineCode(code) => {
                spans.push(Span::styled(code.value.clone(), Theme::secondary()));
            }

            Node::Link(link) => {
                for child in &link.children {
                    Self::render_inline_node(child, spans)?;
                }
            }

            _ => {}
        }

        Ok(())
    }

    fn render_list(
        &self,
        list: &List,
        lines: &mut Vec<Line<'static>>,
        indent_level: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for child in &list.children {
            self.render_node(child, lines, indent_level)?;
        }
        Ok(())
    }

    fn render_list_item(
        &self,
        item: &ListItem,
        lines: &mut Vec<Line<'static>>,
        indent_level: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let indent = "  ".repeat(indent_level);
        let bullet = format!("{} ", BoxChars::DOT);
        let bullet_width = bullet.len();
        let indent_width = indent.len();

        let mut text_content = String::new();
        for child in &item.children {
            if let Node::Paragraph(para) = child {
                for para_child in &para.children {
                    Self::collect_text(para_child, &mut text_content);
                }
                break;
            }
        }

        let available_width = self
            .width
            .unwrap_or(80)
            .saturating_sub(indent_width + bullet_width);

        if available_width > 0 && text_content.len() > available_width {
            let wrapped = wrap(&text_content, available_width);
            for (i, line_text) in wrapped.iter().enumerate() {
                if i == 0 {
                    lines.push(Line::from(vec![
                        Span::raw(indent.clone()),
                        Span::styled(bullet.clone(), Theme::primary()),
                        Span::raw(line_text.to_string()),
                    ]));
                } else {
                    let continuation_indent = " ".repeat(indent_width + bullet_width);
                    lines.push(Line::from(vec![
                        Span::raw(continuation_indent),
                        Span::raw(line_text.to_string()),
                    ]));
                }
            }
        } else {
            lines.push(Line::from(vec![
                Span::raw(indent),
                Span::styled(bullet, Theme::primary()),
                Span::raw(text_content),
            ]));
        }

        for child in &item.children {
            if let Node::List(nested_list) = child {
                self.render_list(nested_list, lines, indent_level + 1)?;
            }
        }

        Ok(())
    }

    fn render_code_block(code: &Code, lines: &mut Vec<Line<'static>>, indent_level: usize) {
        let indent = "  ".repeat(indent_level);
        let lang = code.lang.as_deref().unwrap_or("code");

        lines.push(Line::from(vec![
            Span::raw(indent.clone()),
            Span::styled(
                format!("{} {}", BoxChars::ROUND_TOP_LEFT, lang),
                Theme::border(),
            ),
        ]));

        for line in code.value.lines() {
            lines.push(Line::from(vec![
                Span::raw(indent.clone()),
                Span::styled(format!("{} {}", BoxChars::VERTICAL, line), Theme::muted()),
            ]));
        }

        lines.push(Line::from(vec![
            Span::raw(indent),
            Span::styled(BoxChars::ROUND_BOTTOM_LEFT.to_string(), Theme::border()),
        ]));
    }

    fn collect_text(node: &Node, output: &mut String) {
        match node {
            Node::Text(text) => {
                output.push_str(&text.value);
            }
            Node::Strong(strong) => {
                for child in &strong.children {
                    Self::collect_text(child, output);
                }
            }
            Node::Emphasis(emphasis) => {
                for child in &emphasis.children {
                    Self::collect_text(child, output);
                }
            }
            Node::InlineCode(code) => {
                output.push_str(&code.value);
            }
            Node::Link(link) => {
                for child in &link.children {
                    Self::collect_text(child, output);
                }
            }
            _ => {}
        }
    }
}

impl Widget for MarkdownWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let lines = self.render_to_lines().unwrap_or_else(|_| {
            vec![Line::from(Span::styled(
                "Error rendering markdown",
                Theme::error(),
            ))]
        });

        let mut y = area.y;
        for line in lines {
            if y >= area.y + area.height {
                break;
            }

            buf.set_line(area.x, y, &line, area.width);
            y += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_markdown_widget_creation() {
        let widget = MarkdownWidget::new("# Hello");
        assert_eq!(widget.content, "# Hello");
        assert_eq!(widget.indent_level, 0);
    }

    #[test]
    fn test_markdown_widget_indent() {
        let widget = MarkdownWidget::new("# Hello").indent(2);
        assert_eq!(widget.indent_level, 2);
    }

    #[test]
    fn test_render_headers() -> Result<(), Box<dyn std::error::Error>> {
        let widget = MarkdownWidget::new("# Header 1");
        let lines = widget.render_to_lines()?;
        assert!(!lines.is_empty());
        Ok(())
    }

    #[test]
    fn test_render_lists() -> Result<(), Box<dyn std::error::Error>> {
        let widget = MarkdownWidget::new("- List item");
        let lines = widget.render_to_lines()?;
        assert!(!lines.is_empty());
        Ok(())
    }

    #[test]
    fn test_render_code_block() -> Result<(), Box<dyn std::error::Error>> {
        let widget = MarkdownWidget::new("```rust\nfn main() {}\n```");
        let lines = widget.render_to_lines()?;
        assert!(!lines.is_empty());
        Ok(())
    }

    #[test]
    fn test_render_full_markdown() -> Result<(), Box<dyn std::error::Error>> {
        let markdown = r#"# Title

This is a paragraph with **bold** and `code`.

- Item 1
- Item 2
"#;
        let widget = MarkdownWidget::new(markdown);
        let lines = widget.render_to_lines()?;
        assert!(!lines.is_empty());
        Ok(())
    }

    #[test]
    fn test_paragraph_wrapping_with_width() -> Result<(), Box<dyn std::error::Error>> {
        let long_text = "This is a very long paragraph that should definitely wrap when rendered with a narrow width constraint applied to it.";
        let widget = MarkdownWidget::new(long_text).width(40);
        let lines = widget.render_to_lines()?;
        assert!(lines.len() > 1, "Long text should wrap to multiple lines");
        Ok(())
    }

    #[test]
    fn test_paragraph_no_wrap_without_width() -> Result<(), Box<dyn std::error::Error>> {
        let long_text = "This is a long paragraph without width constraint.";
        let widget = MarkdownWidget::new(long_text);
        let lines = widget.render_to_lines()?;
        assert_eq!(
            lines.len(),
            2,
            "Without width, paragraph should be single line"
        );
        Ok(())
    }

    #[test]
    fn test_short_text_no_wrap() -> Result<(), Box<dyn std::error::Error>> {
        let short_text = "Short text.";
        let widget = MarkdownWidget::new(short_text).width(80);
        let lines = widget.render_to_lines()?;
        assert_eq!(lines.len(), 2, "Short text should not wrap");
        Ok(())
    }

    #[test]
    fn test_nested_lists() -> Result<(), Box<dyn std::error::Error>> {
        let markdown = r#"- Parent item:
  - Child item 1
  - Child item 2
- Another parent
"#;
        let widget = MarkdownWidget::new(markdown);
        let lines = widget.render_to_lines()?;

        assert!(
            lines.len() >= 4,
            "Nested list should render parent and children, got {} lines",
            lines.len()
        );

        let content: String = lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(
            content.contains("Child item 1"),
            "Nested list should contain child items"
        );
        Ok(())
    }

    #[test]
    fn test_links_in_text() -> Result<(), Box<dyn std::error::Error>> {
        let markdown = "Check out [this link](https://example.com) for more info.";
        let widget = MarkdownWidget::new(markdown);
        let lines = widget.render_to_lines()?;

        let content: String = lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("");

        assert!(
            content.contains("this link"),
            "Link text should be rendered"
        );
        Ok(())
    }

    #[test]
    fn test_list_item_wrapping() -> Result<(), Box<dyn std::error::Error>> {
        let markdown = "- This is a very long list item that should wrap to multiple lines when the width is constrained to a narrow value.";
        let widget = MarkdownWidget::new(markdown).width(50);
        let lines = widget.render_to_lines()?;

        assert!(
            lines.len() > 2,
            "Long list item should wrap to multiple lines, got {} lines",
            lines.len()
        );

        if lines.len() > 1 {
            let first_line: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
            let second_line: String = lines[1].spans.iter().map(|s| s.content.as_ref()).collect();

            assert!(first_line.contains("•"), "First line should have bullet");
            assert!(
                second_line.starts_with("  "),
                "Continuation line should be indented"
            );
        }
        Ok(())
    }
}
