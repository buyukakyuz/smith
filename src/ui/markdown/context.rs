#[derive(Debug, Clone, Copy)]
pub struct RenderContext {
    pub indent_level: usize,
    pub width: Option<usize>,
}

impl RenderContext {
    const INDENT_STR: &'static str = "  ";

    pub fn new(indent_level: usize, width: Option<usize>) -> Self {
        Self {
            indent_level,
            width,
        }
    }

    pub fn indent(&self) -> String {
        Self::INDENT_STR.repeat(self.indent_level)
    }

    pub fn indent_width(&self) -> usize {
        self.indent_level * Self::INDENT_STR.len()
    }

    pub fn available_width(&self) -> usize {
        self.width.unwrap_or(80).saturating_sub(self.indent_width())
    }

    pub fn nested(&self) -> Self {
        Self {
            indent_level: self.indent_level + 1,
            ..*self
        }
    }
}
