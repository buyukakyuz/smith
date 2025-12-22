#![allow(clippy::cast_possible_truncation)]

use crate::tui::app::SLASH_COMMANDS;
use crate::ui::theme::Theme;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};
use tui_textarea::TextArea;

const PASTE_PLACEHOLDER_THRESHOLD: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputAction {
    Continue,
    Submit(String),
    HistoryPrev,
    HistoryNext,
    Clear,
}

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
        let mut textarea = TextArea::default();
        textarea.set_placeholder_text("");
        textarea.set_cursor_line_style(Style::default());
        textarea.set_cursor_style(Theme::white());

        Self {
            textarea,
            suggestions: Vec::new(),
            show_suggestions: false,
            selected_suggestion: 0,
            pasted_blocks: Vec::new(),
            next_paste_id: 0,
        }
    }

    fn make_placeholder(id: usize, char_count: usize) -> String {
        format!("[paste#{id}:{char_count}]")
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
        self.textarea.set_cursor_line_style(Style::default());
        self.textarea.set_cursor_style(Theme::white());
        self.textarea.move_cursor(tui_textarea::CursorMove::End);
        self.pasted_blocks.clear();
    }

    pub fn clear(&mut self) {
        self.textarea = TextArea::default();
        self.textarea.set_placeholder_text("");
        self.textarea.set_cursor_line_style(Style::default());
        self.textarea.set_cursor_style(Theme::white());
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

    pub fn handle_key(&mut self, key: KeyEvent) -> InputAction {
        match (key.code, key.modifiers) {
            (KeyCode::Char('/'), KeyModifiers::NONE) if self.is_empty() => {
                self.textarea.input(key);
                self.update_suggestions();
                InputAction::Continue
            }

            (KeyCode::Tab, KeyModifiers::NONE) => {
                if self.show_suggestions && !self.suggestions.is_empty() {
                    self.apply_selected_suggestion();
                } else {
                    self.textarea.input(key);
                }
                InputAction::Continue
            }

            (KeyCode::BackTab, _) => {
                if self.show_suggestions && !self.suggestions.is_empty() {
                    if self.selected_suggestion > 0 {
                        self.selected_suggestion -= 1;
                    } else {
                        self.selected_suggestion = self.suggestions.len() - 1;
                    }
                }
                InputAction::Continue
            }

            (KeyCode::Down, KeyModifiers::NONE) => {
                if self.show_suggestions && !self.suggestions.is_empty() {
                    self.selected_suggestion =
                        (self.selected_suggestion + 1) % self.suggestions.len();
                    InputAction::Continue
                } else if self.is_empty() {
                    InputAction::HistoryNext
                } else {
                    self.textarea.input(key);
                    InputAction::Continue
                }
            }

            (KeyCode::Up, KeyModifiers::NONE) => {
                if self.show_suggestions && !self.suggestions.is_empty() {
                    if self.selected_suggestion > 0 {
                        self.selected_suggestion -= 1;
                    } else {
                        self.selected_suggestion = self.suggestions.len() - 1;
                    }
                    InputAction::Continue
                } else if self.is_empty() {
                    InputAction::HistoryPrev
                } else {
                    self.textarea.input(key);
                    InputAction::Continue
                }
            }

            (KeyCode::Enter, KeyModifiers::SHIFT) => {
                self.textarea.insert_newline();
                self.hide_suggestions();
                InputAction::Continue
            }

            (KeyCode::Enter, KeyModifiers::NONE) => {
                if self.show_suggestions && !self.suggestions.is_empty() {
                    self.suggestions
                        .get(self.selected_suggestion)
                        .cloned()
                        .map_or(InputAction::Continue, |suggestion| {
                            self.hide_suggestions();
                            self.clear();
                            InputAction::Submit(suggestion)
                        })
                } else if self.is_empty() {
                    InputAction::Continue
                } else {
                    let text = self.take();
                    InputAction::Submit(text)
                }
            }

            (KeyCode::Char('k' | 'u'), KeyModifiers::CONTROL) => {
                self.clear();
                InputAction::Clear
            }

            (KeyCode::Char(_), KeyModifiers::NONE | KeyModifiers::SHIFT)
            | (KeyCode::Backspace, _) => {
                self.textarea.input(key);
                if self.text().starts_with('/') {
                    self.update_suggestions();
                } else {
                    self.hide_suggestions();
                }
                InputAction::Continue
            }

            _ => {
                self.textarea.input(key);
                InputAction::Continue
            }
        }
    }

    pub fn handle_paste(&mut self, text: String) -> InputAction {
        self.hide_suggestions();

        if text.len() <= PASTE_PLACEHOLDER_THRESHOLD {
            self.insert_text_direct(&text);
            return InputAction::Continue;
        }

        let paste_id = self.next_paste_id;
        self.next_paste_id += 1;

        let char_count = text.len();
        let placeholder = Self::make_placeholder(paste_id, char_count);

        self.pasted_blocks.push(PastedBlock {
            content: text,
            placeholder_id: paste_id,
        });

        for ch in placeholder.chars() {
            self.textarea.insert_char(ch);
        }

        InputAction::Continue
    }

    fn insert_text_direct(&mut self, text: &str) {
        let lines: Vec<&str> = text.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            for ch in line.chars() {
                self.textarea.insert_char(ch);
            }
            if i < lines.len() - 1 {
                self.textarea.insert_newline();
            }
        }
        if text.ends_with('\n') {
            self.textarea.insert_newline();
        }
    }

    fn update_suggestions(&mut self) {
        let text = self.text();
        if text.starts_with('/') {
            self.suggestions = SLASH_COMMANDS
                .iter()
                .filter(|cmd| cmd.starts_with(&text))
                .map(|s| (*s).to_string())
                .collect();

            self.show_suggestions = !self.suggestions.is_empty();
            self.selected_suggestion = 0;
        } else {
            self.hide_suggestions();
        }
    }

    fn apply_selected_suggestion(&mut self) {
        if let Some(suggestion) = self.suggestions.get(self.selected_suggestion) {
            self.set_text(&format!("{suggestion} "));
            self.hide_suggestions();
        }
    }

    fn hide_suggestions(&mut self) {
        self.show_suggestions = false;
        self.suggestions.clear();
        self.selected_suggestion = 0;
    }

    pub fn render(&mut self, area: Rect, frame: &mut Frame) {
        let buf = frame.buffer_mut();

        let separator_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 1,
        };
        let separator = Line::from(Span::styled(
            "─".repeat(area.width as usize),
            Theme::border(),
        ));
        Paragraph::new(separator).render(separator_area, buf);

        let input_area = Rect {
            x: area.x,
            y: area.y + 1,
            width: area.width,
            height: area.height.saturating_sub(1),
        };

        let prefix_area = Rect {
            x: input_area.x,
            y: input_area.y,
            width: 2,
            height: 1,
        };
        let prefix = Line::from(Span::styled("> ", Theme::white()));
        Paragraph::new(prefix).render(prefix_area, buf);

        let hint_text = "↵ send";
        let hint_width = hint_text.len() as u16;
        if !self.is_empty() {
            let hint_area = Rect {
                x: input_area.x + input_area.width - hint_width - 1,
                y: input_area.y,
                width: hint_width,
                height: 1,
            };
            let hint = Line::from(Span::styled(hint_text, Theme::muted()));
            Paragraph::new(hint).render(hint_area, buf);
        }

        let textarea_area = Rect {
            x: input_area.x + 2,
            y: input_area.y,
            width: input_area.width.saturating_sub(2 + hint_width + 2),
            height: input_area.height,
        };

        self.textarea
            .set_block(Block::default().borders(Borders::NONE));

        frame.render_widget(&self.textarea, textarea_area);

        let (cursor_row, cursor_col) = self.textarea.cursor();
        frame.set_cursor_position(Position::new(
            textarea_area.x + cursor_col as u16,
            textarea_area.y + cursor_row as u16,
        ));

        if self.show_suggestions && !self.suggestions.is_empty() {
            self.render_suggestions(area, frame.buffer_mut());
        }
    }

    fn render_suggestions(&self, area: Rect, buf: &mut Buffer) {
        let suggestion_height = self.suggestions.len().min(5) as u16 + 2;
        if area.y < suggestion_height {
            return;
        }

        let suggestions_area = Rect {
            x: area.x + 2,
            y: area.y.saturating_sub(suggestion_height),
            width: 30.min(area.width.saturating_sub(4)),
            height: suggestion_height,
        };

        let lines: Vec<Line> = self
            .suggestions
            .iter()
            .enumerate()
            .map(|(i, cmd)| {
                let style = if i == self.selected_suggestion {
                    Theme::primary_bold()
                } else {
                    Theme::muted()
                };
                Line::from(vec![
                    Span::raw(" "),
                    Span::styled(cmd, style),
                    if i == self.selected_suggestion {
                        Span::styled(" ←", Theme::primary())
                    } else {
                        Span::raw("  ")
                    },
                ])
            })
            .collect();

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Theme::primary())
            .border_set(ratatui::symbols::border::ROUNDED)
            .title(" Commands ");

        let paragraph = Paragraph::new(lines).block(block);
        paragraph.render(suggestions_area, buf);
    }
}

impl Default for InputWidget<'_> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_widget_creation() {
        let widget = InputWidget::new();
        assert!(widget.is_empty());
        assert!(!widget.show_suggestions);
    }

    #[test]
    fn test_slash_command_suggestions() {
        let mut widget = InputWidget::new();
        let action = widget.handle_key(KeyEvent::from(KeyCode::Char('/')));
        assert_eq!(action, InputAction::Continue);
        assert!(widget.show_suggestions);
        assert!(!widget.suggestions.is_empty());
    }

    #[test]
    fn test_submit_action() {
        let mut widget = InputWidget::new();
        widget.handle_key(KeyEvent::from(KeyCode::Char('h')));
        widget.handle_key(KeyEvent::from(KeyCode::Char('i')));

        let action = widget.handle_key(KeyEvent::from(KeyCode::Enter));
        assert_eq!(action, InputAction::Submit("hi".to_string()));
        assert!(widget.is_empty());
    }

    #[test]
    fn test_multiline_input() {
        let mut widget = InputWidget::new();
        widget.handle_key(KeyEvent::from(KeyCode::Char('l')));
        widget.handle_key(KeyEvent::from(KeyCode::Char('1')));

        let action = widget.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT));
        assert_eq!(action, InputAction::Continue);

        widget.handle_key(KeyEvent::from(KeyCode::Char('l')));
        widget.handle_key(KeyEvent::from(KeyCode::Char('2')));

        assert_eq!(widget.text(), "l1\nl2");
    }

    #[test]
    fn test_clear_action() {
        let mut widget = InputWidget::new();
        widget.handle_key(KeyEvent::from(KeyCode::Char('t')));
        widget.handle_key(KeyEvent::from(KeyCode::Char('e')));
        assert!(!widget.is_empty());

        let action = widget.handle_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL));
        assert_eq!(action, InputAction::Clear);
        assert!(widget.is_empty());
    }

    #[test]
    fn test_history_navigation() {
        let mut widget = InputWidget::new();
        let action = widget.handle_key(KeyEvent::from(KeyCode::Up));
        assert_eq!(action, InputAction::HistoryPrev);
    }

    #[test]
    fn test_autocomplete_filtering() {
        let mut widget = InputWidget::new();
        widget.handle_key(KeyEvent::from(KeyCode::Char('/')));
        widget.handle_key(KeyEvent::from(KeyCode::Char('h')));

        assert!(widget.show_suggestions);
        assert!(widget.suggestions.contains(&"/help".to_string()));
        assert!(!widget.suggestions.contains(&"/exit".to_string()));
    }

    #[test]
    fn test_enter_with_suggestion_submits_selected_command() {
        let mut widget = InputWidget::new();
        widget.handle_key(KeyEvent::from(KeyCode::Char('/')));
        assert!(widget.show_suggestions);
        assert!(!widget.suggestions.is_empty());

        let action = widget.handle_key(KeyEvent::from(KeyCode::Enter));
        assert_eq!(action, InputAction::Submit("/help".to_string()));
        assert!(widget.is_empty());
    }

    #[test]
    fn test_enter_with_arrow_navigated_suggestion() {
        let mut widget = InputWidget::new();
        widget.handle_key(KeyEvent::from(KeyCode::Char('/')));

        widget.handle_key(KeyEvent::from(KeyCode::Down));
        assert_eq!(widget.selected_suggestion, 1);

        let action = widget.handle_key(KeyEvent::from(KeyCode::Enter));
        assert_eq!(action, InputAction::Submit("/exit".to_string()));
    }
}
