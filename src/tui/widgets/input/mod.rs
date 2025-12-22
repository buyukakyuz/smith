mod action;
mod key_handler;
mod render;

pub use action::InputAction;

use crate::ui::theme::Theme;
use ratatui::style::Style;
use tui_textarea::TextArea;

const PASTE_PLACEHOLDER_THRESHOLD: usize = 200;

#[derive(Debug, Clone)]
struct PastedBlock {
    content: String,
    placeholder_id: usize,
}

pub struct InputWidget<'a> {
    textarea: TextArea<'a>,
    suggestions: Vec<String>,
    show_suggestions: bool,
    selected_suggestion: usize,
    pasted_blocks: Vec<PastedBlock>,
    next_paste_id: usize,
}

impl InputWidget<'_> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            textarea: Self::create_textarea(),
            suggestions: Vec::new(),
            show_suggestions: false,
            selected_suggestion: 0,
            pasted_blocks: Vec::new(),
            next_paste_id: 0,
        }
    }

    #[must_use]
    pub fn text(&self) -> String {
        let mut result = self.textarea.lines().join("\n");

        for block in &self.pasted_blocks {
            let placeholder = Self::make_placeholder(block.placeholder_id, block.content.len());
            result = result.replace(&placeholder, &block.content);
        }

        result
    }

    pub fn set_text(&mut self, text: &str) {
        let lines: Vec<String> = text.lines().map(ToString::to_string).collect();
        self.textarea = TextArea::new(lines);
        self.configure_textarea();
        self.textarea.move_cursor(tui_textarea::CursorMove::End);
        self.pasted_blocks.clear();
    }

    pub fn clear(&mut self) {
        self.textarea = Self::create_textarea();
        self.suggestions.clear();
        self.show_suggestions = false;
        self.pasted_blocks.clear();
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.textarea.lines().iter().all(String::is_empty)
    }

    pub fn take(&mut self) -> String {
        let text = self.text();
        self.clear();
        text
    }

    fn create_textarea() -> TextArea<'static> {
        let mut textarea = TextArea::default();
        textarea.set_placeholder_text("");
        textarea.set_cursor_line_style(Style::default());
        textarea.set_cursor_style(Theme::white());
        textarea
    }

    fn configure_textarea(&mut self) {
        self.textarea.set_cursor_line_style(Style::default());
        self.textarea.set_cursor_style(Theme::white());
    }

    fn make_placeholder(id: usize, char_count: usize) -> String {
        format!("[paste#{id}:{char_count}]")
    }
}

impl Default for InputWidget<'_> {
    fn default() -> Self {
        Self::new()
    }
}
