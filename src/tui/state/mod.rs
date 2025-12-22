mod history;
mod messages;
mod modals;
mod streaming;
mod tools;

pub use history::InputHistory;
pub use modals::{ModelPickerModal, PermissionModal, PickerModel};
pub use tools::ToolExecution;

use crate::permission::types::{PermissionRequest, PermissionResponse};
use crate::tui::widgets::{ChatMessage, ScrollState};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::oneshot;

pub struct AppState {
    pub should_quit: bool,
    pub frame: usize,
    pub spinner_frame: usize,
    pub is_processing: bool,
    pub history: InputHistory,
    pub messages: Vec<ChatMessage>,
    pub scroll: ScrollState,
    pub streaming_response: Option<String>,
    pub active_tools: HashMap<String, ToolExecution>,
    pub permission_modal: Option<PermissionModal>,
    pub model_picker_modal: Option<ModelPickerModal>,

    spinner_last_update: Option<Instant>,
    request_start: Option<Instant>,
}

impl AppState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            should_quit: false,
            frame: 0,
            spinner_frame: 0,
            spinner_last_update: None,
            is_processing: false,
            request_start: None,
            history: InputHistory::new(),
            messages: Vec::new(),
            scroll: ScrollState::new(),
            streaming_response: None,
            active_tools: HashMap::new(),
            permission_modal: None,
            model_picker_modal: None,
        }
    }

    pub fn tick(&mut self) {
        self.frame = self.frame.wrapping_add(1);
        self.update_spinner();
    }

    fn update_spinner(&mut self) {
        const SPINNER_INTERVAL: Duration = Duration::from_millis(80);

        let now = Instant::now();
        match self.spinner_last_update {
            Some(last) if now.duration_since(last) >= SPINNER_INTERVAL => {
                self.spinner_frame = self.spinner_frame.wrapping_add(1);
                self.spinner_last_update = Some(now);
            }
            None => {
                self.spinner_last_update = Some(now);
            }
            _ => {}
        }
    }

    pub const fn quit(&mut self) {
        self.should_quit = true;
    }

    pub fn start_processing(&mut self) {
        self.is_processing = true;
        self.request_start = Some(Instant::now());
    }

    pub const fn stop_processing(&mut self) {
        self.is_processing = false;
        self.request_start = None;
    }

    #[must_use]
    pub fn elapsed(&self) -> Option<Duration> {
        self.request_start.map(|start| start.elapsed())
    }

    #[must_use]
    pub const fn has_modal(&self) -> bool {
        self.permission_modal.is_some() || self.model_picker_modal.is_some()
    }

    #[must_use]
    pub const fn has_model_picker(&self) -> bool {
        self.model_picker_modal.is_some()
    }

    pub fn show_model_picker(&mut self) {
        self.model_picker_modal = Some(ModelPickerModal::new());
    }

    pub fn model_picker_confirm(&mut self) -> Option<String> {
        self.model_picker_modal
            .take()
            .and_then(|m| m.selected_model())
    }

    pub fn model_picker_cancel(&mut self) {
        self.model_picker_modal = None;
    }

    pub fn model_picker_select_prev(&mut self) {
        if let Some(modal) = &mut self.model_picker_modal {
            modal.select_prev();
        }
    }

    pub fn model_picker_select_next(&mut self) {
        if let Some(modal) = &mut self.model_picker_modal {
            modal.select_next();
        }
    }

    pub fn show_permission_modal(
        &mut self,
        request: PermissionRequest,
        response_tx: oneshot::Sender<PermissionResponse>,
    ) {
        self.permission_modal = Some(PermissionModal::new(request, response_tx));
    }

    pub fn permission_confirm(&mut self) -> Option<String> {
        self.permission_modal.take().and_then(|m| m.confirm())
    }

    pub fn permission_cancel(&mut self) -> bool {
        self.permission_modal.take().map(|m| m.cancel()).is_some()
    }

    #[must_use]
    pub fn permission_in_input_mode(&self) -> bool {
        self.permission_modal
            .as_ref()
            .map_or(false, |m| m.is_input_mode())
    }

    pub fn permission_select_prev(&mut self) {
        if let Some(modal) = &mut self.permission_modal {
            modal.select_prev();
        }
    }

    pub fn permission_select_next(&mut self) {
        if let Some(modal) = &mut self.permission_modal {
            modal.select_next();
        }
    }

    pub fn permission_input_char(&mut self, c: char) {
        if let Some(modal) = &mut self.permission_modal {
            modal.input_char(c);
        }
    }

    pub fn permission_input_backspace(&mut self) {
        if let Some(modal) = &mut self.permission_modal {
            modal.input_backspace();
        }
    }

    pub fn permission_set_selection(&mut self, index: usize) {
        if let Some(modal) = &mut self.permission_modal {
            modal.set_selection(index);
        }
    }

    pub const fn scroll_up(&mut self, lines: usize) {
        self.scroll.scroll_up(lines);
    }

    pub fn scroll_down(&mut self, lines: usize) {
        self.scroll.scroll_down(lines);
    }

    pub const fn scroll_to_top(&mut self) {
        self.scroll.scroll_to_top();
    }

    pub const fn scroll_to_bottom(&mut self) {
        self.scroll.scroll_to_bottom();
    }

    pub fn add_to_history(&mut self, input: String) {
        self.history.push(input);
    }

    pub fn history_prev(&mut self) -> Option<String> {
        self.history.prev()
    }

    pub fn history_next(&mut self) -> Option<String> {
        self.history.next()
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_state_is_not_quitting() {
        let state = AppState::new();
        assert!(!state.should_quit);
        assert_eq!(state.frame, 0);
        assert!(!state.is_processing);
    }

    #[test]
    fn tick_increments_frame() {
        let mut state = AppState::new();
        state.tick();
        assert_eq!(state.frame, 1);
        state.tick();
        assert_eq!(state.frame, 2);
    }

    #[test]
    fn processing_tracks_elapsed_time() {
        let mut state = AppState::new();
        assert!(state.elapsed().is_none());

        state.start_processing();
        assert!(state.is_processing);
        assert!(state.elapsed().is_some());

        state.stop_processing();
        assert!(!state.is_processing);
        assert!(state.elapsed().is_none());
    }
}
