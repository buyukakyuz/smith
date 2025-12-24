#![allow(clippy::cast_possible_truncation)]
use markdown::{ParseOptions, to_mdast};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    text::{Line, Span},
    widgets::Widget,
};

use crate::ui::theme::Theme;

use super::block::render_node;
use super::context::RenderContext;
use super::error::MarkdownError;

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

    pub fn render_to_lines(&self) -> Result<Vec<Line<'static>>, MarkdownError> {
        let ast = to_mdast(&self.content, &ParseOptions::default())
            .map_err(|e| MarkdownError::Parse(e.to_string()))?;

        let ctx = RenderContext::new(self.indent_level, self.width);

        Ok(render_node(&ast, ctx))
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

        for (y_offset, line) in lines.iter().take(area.height as usize).enumerate() {
            buf.set_line(area.x, area.y + y_offset as u16, line, area.width);
        }
    }
}
