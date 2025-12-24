use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{InputAction, InputWidget, PASTE_PLACEHOLDER_THRESHOLD, PastedBlock};
use crate::tui::app::SLASH_COMMANDS;

impl InputWidget<'_> {
    pub fn handle_key(&mut self, key: KeyEvent) -> InputAction {
        match (key.code, key.modifiers) {
            (KeyCode::Char('/'), KeyModifiers::NONE) if self.is_empty() => {
                self.textarea.input(key);
                self.update_suggestions();
                InputAction::Continue
            }

            (KeyCode::Tab, KeyModifiers::NONE) => self.handle_tab(),
            (KeyCode::BackTab, _) => self.handle_backtab(),

            (KeyCode::Down, KeyModifiers::NONE) => self.handle_down(),
            (KeyCode::Up, KeyModifiers::NONE) => self.handle_up(),

            (KeyCode::Enter, KeyModifiers::SHIFT) => {
                self.textarea.insert_newline();
                self.hide_suggestions();
                InputAction::Continue
            }
            (KeyCode::Enter, KeyModifiers::NONE) => self.handle_enter(),

            (KeyCode::Char('k' | 'u'), KeyModifiers::CONTROL) => {
                self.clear();
                InputAction::Clear
            }

            (KeyCode::Char(_), KeyModifiers::NONE | KeyModifiers::SHIFT)
            | (KeyCode::Backspace, _) => {
                self.textarea.input(key);
                self.update_suggestions_if_slash();
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
        } else {
            self.insert_paste_placeholder(text);
        }

        InputAction::Continue
    }

    fn handle_tab(&mut self) -> InputAction {
        if self.show_suggestions && !self.suggestions.is_empty() {
            self.apply_selected_suggestion();
        } else {
            self.textarea
                .input(crossterm::event::KeyEvent::from(KeyCode::Tab));
        }
        InputAction::Continue
    }

    fn handle_backtab(&mut self) -> InputAction {
        if self.show_suggestions && !self.suggestions.is_empty() {
            self.select_previous_suggestion();
        }
        InputAction::Continue
    }

    fn handle_down(&mut self) -> InputAction {
        if self.show_suggestions && !self.suggestions.is_empty() {
            self.select_next_suggestion();
            InputAction::Continue
        } else if self.is_empty() {
            InputAction::HistoryNext
        } else {
            self.textarea
                .input(crossterm::event::KeyEvent::from(KeyCode::Down));
            InputAction::Continue
        }
    }

    fn handle_up(&mut self) -> InputAction {
        if self.show_suggestions && !self.suggestions.is_empty() {
            self.select_previous_suggestion();
            InputAction::Continue
        } else if self.is_empty() {
            InputAction::HistoryPrev
        } else {
            self.textarea
                .input(crossterm::event::KeyEvent::from(KeyCode::Up));
            InputAction::Continue
        }
    }

    fn handle_enter(&mut self) -> InputAction {
        if self.show_suggestions && !self.suggestions.is_empty() {
            self.submit_selected_suggestion()
        } else if self.is_empty() {
            InputAction::Continue
        } else {
            InputAction::Submit(self.take())
        }
    }

    fn update_suggestions_if_slash(&mut self) {
        if self.text().starts_with('/') {
            self.update_suggestions();
        } else {
            self.hide_suggestions();
        }
    }

    fn update_suggestions(&mut self) {
        let text = self.text();

        self.suggestions = SLASH_COMMANDS
            .iter()
            .filter(|cmd| cmd.starts_with(&text))
            .map(|s| (*s).to_string())
            .collect();

        self.show_suggestions = !self.suggestions.is_empty();
        self.selected_suggestion = 0;
    }

    fn hide_suggestions(&mut self) {
        self.show_suggestions = false;
        self.suggestions.clear();
        self.selected_suggestion = 0;
    }

    const fn select_next_suggestion(&mut self) {
        self.selected_suggestion = (self.selected_suggestion + 1) % self.suggestions.len();
    }

    const fn select_previous_suggestion(&mut self) {
        if self.selected_suggestion > 0 {
            self.selected_suggestion -= 1;
        } else {
            self.selected_suggestion = self.suggestions.len().saturating_sub(1);
        }
    }

    fn apply_selected_suggestion(&mut self) {
        if let Some(suggestion) = self.suggestions.get(self.selected_suggestion) {
            self.set_text(&format!("{suggestion} "));
            self.hide_suggestions();
        }
    }

    fn submit_selected_suggestion(&mut self) -> InputAction {
        self.suggestions
            .get(self.selected_suggestion)
            .cloned()
            .map_or(InputAction::Continue, |suggestion| {
                self.hide_suggestions();
                self.clear();
                InputAction::Submit(suggestion)
            })
    }

    fn insert_text_direct(&mut self, text: &str) {
        for (i, line) in text.lines().enumerate() {
            if i > 0 {
                self.textarea.insert_newline();
            }
            for ch in line.chars() {
                self.textarea.insert_char(ch);
            }
        }

        if text.ends_with('\n') {
            self.textarea.insert_newline();
        }
    }

    fn insert_paste_placeholder(&mut self, content: String) {
        let paste_id = self.next_paste_id;
        self.next_paste_id += 1;

        let placeholder = Self::make_placeholder(paste_id, content.len());

        self.pasted_blocks.push(PastedBlock {
            content,
            placeholder_id: paste_id,
        });

        for ch in placeholder.chars() {
            self.textarea.insert_char(ch);
        }
    }
}
