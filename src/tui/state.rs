use crate::config::ModelRegistry;
use crate::core::metadata;
use crate::permission::types::{PermissionRequest, PermissionResponse};
use crate::tools::result::ToolResult;
use crate::tools::{ToolState, ToolType};
use crate::tui::widgets::{ChatMessage, ScrollState};
use crate::ui::output_widget::MessageLevel;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::oneshot;

#[derive(Debug, Clone)]
pub struct ToolExecution {
    pub tool_type: ToolType,
    pub input: String,
    pub started_at: Instant,
}

pub struct AppState {
    pub should_quit: bool,
    pub frame: usize,
    pub spinner_frame: usize,
    pub spinner_last_update: Option<Instant>,
    pub is_processing: bool,
    pub request_start: Option<Instant>,
    pub input_history: Vec<String>,
    pub history_index: Option<usize>,
    pub messages: Vec<ChatMessage>,
    pub scroll: ScrollState,
    pub streaming_response: Option<String>,
    pub active_tools: HashMap<String, ToolExecution>,
    pub permission_modal: Option<PermissionModal>,
    pub model_picker_modal: Option<ModelPickerModal>,
}

pub struct PermissionModal {
    pub request: PermissionRequest,
    pub selected: usize,
    pub response_tx: oneshot::Sender<PermissionResponse>,
    pub input_mode: bool,
    pub feedback_input: String,
}

#[derive(Clone)]
pub struct PickerModel {
    pub id: String,
    pub name: String,
}

pub struct ModelPickerModal {
    pub models: Vec<(String, Vec<PickerModel>)>,
    pub selected: usize,
    pub total_models: usize,
}

impl ModelPickerModal {
    #[must_use]
    pub fn new() -> Self {
        let registry = ModelRegistry::load();
        let models: Vec<_> = registry
            .all_models_by_provider()
            .into_iter()
            .map(|(provider, models)| {
                let picker_models: Vec<_> = models
                    .iter()
                    .map(|m| PickerModel {
                        id: m.id.clone(),
                        name: m.name.clone(),
                    })
                    .collect();
                (provider.to_string(), picker_models)
            })
            .collect();
        let total_models = models.iter().map(|(_, m)| m.len()).sum();
        Self {
            models,
            selected: 0,
            total_models,
        }
    }

    #[must_use]
    pub fn selected_model(&self) -> Option<String> {
        let mut idx = 0;
        for (_, models) in &self.models {
            for model in models {
                if idx == self.selected {
                    return Some(model.id.clone());
                }
                idx += 1;
            }
        }
        None
    }

    pub fn select_prev(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn select_next(&mut self) {
        if self.selected + 1 < self.total_models {
            self.selected += 1;
        }
    }
}

impl Default for ModelPickerModal {
    fn default() -> Self {
        Self::new()
    }
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
            input_history: Vec::new(),
            history_index: None,
            messages: Vec::new(),
            scroll: ScrollState::new(),
            streaming_response: None,
            active_tools: HashMap::new(),
            permission_modal: None,
            model_picker_modal: None,
        }
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

    pub fn model_picker_confirm(&mut self) -> Option<String> {
        if let Some(modal) = self.model_picker_modal.take() {
            modal.selected_model()
        } else {
            None
        }
    }

    pub fn model_picker_cancel(&mut self) {
        self.model_picker_modal = None;
    }

    pub fn show_permission_modal(
        &mut self,
        request: PermissionRequest,
        response_tx: oneshot::Sender<PermissionResponse>,
    ) {
        self.permission_modal = Some(PermissionModal {
            request,
            selected: 0,
            response_tx,
            input_mode: false,
            feedback_input: String::new(),
        });
    }

    #[must_use]
    pub fn permission_in_input_mode(&self) -> bool {
        self.permission_modal.as_ref().is_some_and(|m| m.input_mode)
    }

    pub fn permission_input_char(&mut self, c: char) {
        if let Some(modal) = &mut self.permission_modal {
            modal.feedback_input.push(c);
        }
    }

    pub fn permission_input_backspace(&mut self) {
        if let Some(modal) = &mut self.permission_modal {
            modal.feedback_input.pop();
        }
    }

    pub fn permission_select_prev(&mut self) {
        if let Some(modal) = &mut self.permission_modal {
            let new_selected = modal.selected.saturating_sub(1);
            modal.selected = new_selected;

            modal.input_mode = new_selected == 2;
            if !modal.input_mode {
                modal.feedback_input.clear();
            }
        }
    }

    pub fn permission_select_next(&mut self) {
        if let Some(modal) = &mut self.permission_modal {
            let new_selected = (modal.selected + 1).min(2);
            modal.selected = new_selected;
            modal.input_mode = new_selected == 2;
        }
    }

    pub fn permission_confirm(&mut self) -> Option<String> {
        if let Some(modal) = self.permission_modal.take() {
            let (response, feedback) = match modal.selected {
                0 => (PermissionResponse::AllowOnce, None),
                1 => (PermissionResponse::AllowSession, None),
                _ => {
                    let feedback = if modal.feedback_input.trim().is_empty() {
                        "User declined the operation. Please ask what to do instead.".to_string()
                    } else {
                        modal.feedback_input
                    };
                    (
                        PermissionResponse::TellModelDifferently(feedback.clone()),
                        Some(feedback),
                    )
                }
            };
            let _ = modal.response_tx.send(response);
            feedback
        } else {
            None
        }
    }

    pub fn permission_cancel(&mut self) -> bool {
        if let Some(modal) = self.permission_modal.take() {
            let _ = modal
                .response_tx
                .send(PermissionResponse::TellModelDifferently(
                    "User cancelled the operation".to_string(),
                ));
            true
        } else {
            false
        }
    }

    pub fn tick(&mut self) {
        self.frame = self.frame.wrapping_add(1);

        let now = Instant::now();
        if let Some(last_update) = self.spinner_last_update {
            if now.duration_since(last_update) >= Duration::from_millis(80) {
                self.spinner_frame = self.spinner_frame.wrapping_add(1);
                self.spinner_last_update = Some(now);
            }
        } else {
            self.spinner_last_update = Some(now);
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
    pub fn elapsed(&self) -> Option<std::time::Duration> {
        self.request_start.map(|start| start.elapsed())
    }

    pub fn add_to_history(&mut self, input: String) {
        if !input.trim().is_empty() && self.input_history.last() != Some(&input) {
            self.input_history.push(input);
            if self.input_history.len() > 100 {
                self.input_history.remove(0);
            }
        }
        self.history_index = None;
    }

    #[must_use]
    pub fn history_prev(&mut self) -> Option<String> {
        if self.input_history.is_empty() {
            return None;
        }

        let new_index = match self.history_index {
            None => Some(self.input_history.len() - 1),
            Some(0) => Some(0),
            Some(i) => Some(i - 1),
        };

        self.history_index = new_index;
        new_index.and_then(|i| self.input_history.get(i).cloned())
    }

    #[must_use]
    pub fn history_next(&mut self) -> Option<String> {
        match self.history_index {
            None => None,
            Some(i) if i >= self.input_history.len() - 1 => {
                self.history_index = None;
                None
            }
            Some(i) => {
                self.history_index = Some(i + 1);
                self.input_history.get(i + 1).cloned()
            }
        }
    }

    pub fn add_user_message(&mut self, text: String) {
        self.messages.push(ChatMessage::User(text));
        self.scroll.reset_manual_scroll();
    }

    pub fn add_assistant_message(&mut self, text: String) {
        self.messages.push(ChatMessage::Assistant(text));
        self.scroll.reset_manual_scroll();
    }

    pub fn add_system_message(&mut self, text: String) {
        self.add_system_message_with_level(text, MessageLevel::Info);
    }

    pub fn add_system_message_with_level(&mut self, text: String, level: MessageLevel) {
        self.messages.push(ChatMessage::System { text, level });
        self.scroll.reset_manual_scroll();
    }

    #[must_use]
    pub fn messages_with_streaming(&self) -> Vec<ChatMessage> {
        let mut all_messages = self.messages.clone();

        for execution in self.active_tools.values() {
            all_messages.push(ChatMessage::ToolExecution {
                tool_type: execution.tool_type.clone(),
                input: execution.input.clone(),
                output: None,
                elapsed: Some(execution.started_at.elapsed()),
                state: ToolState::InProgress,
            });
        }

        if let Some(streaming_text) = &self.streaming_response {
            all_messages.push(ChatMessage::StreamingAssistant(streaming_text.clone()));
        }

        all_messages
    }

    pub fn clear_messages(&mut self) {
        self.messages.clear();
        self.scroll = ScrollState::new();
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

    pub fn append_streaming(&mut self, chunk: &str) {
        match &mut self.streaming_response {
            Some(existing) => existing.push_str(chunk),
            None => self.streaming_response = Some(chunk.to_string()),
        }
        if !self.scroll.is_manual_scroll() {
            self.scroll.scroll_to_bottom();
        }
    }

    pub fn finalize_streaming(&mut self) -> String {
        self.streaming_response.take().unwrap_or_default()
    }

    pub fn start_tool(&mut self, name: &str, input: String) {
        self.active_tools.insert(
            name.to_string(),
            ToolExecution {
                tool_type: ToolType::from_name(name),
                input,
                started_at: Instant::now(),
            },
        );
    }

    pub fn complete_tool(&mut self, name: &str, result: ToolResult) {
        if let Some(execution) = self.active_tools.remove(name) {
            let duration = execution.started_at.elapsed();
            let output = result.output().map(metadata::strip);

            self.messages.push(ChatMessage::ToolExecution {
                tool_type: execution.tool_type,
                input: execution.input,
                output,
                elapsed: Some(duration),
                state: ToolState::Success,
            });
            self.scroll.reset_manual_scroll();
        }
    }

    pub fn fail_tool(&mut self, name: &str, error: String) {
        if let Some(execution) = self.active_tools.remove(name) {
            let duration = execution.started_at.elapsed();

            self.messages.push(ChatMessage::ToolExecution {
                tool_type: execution.tool_type,
                input: execution.input,
                output: Some(format!("Error: {error}")),
                elapsed: Some(duration),
                state: ToolState::Error,
            });
            self.scroll.reset_manual_scroll();
        }
    }

    pub fn add_file_diff(&mut self, path: String, old_content: String, new_content: String) {
        self.messages.push(ChatMessage::FileDiff {
            path,
            old_content,
            new_content,
            collapsed: false,
        });
        self.scroll.reset_manual_scroll();
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
    fn test_state_creation() {
        let state = AppState::new();
        assert!(!state.should_quit);
        assert_eq!(state.frame, 0);
        assert!(!state.is_processing);
    }

    #[test]
    fn test_tick() {
        let mut state = AppState::new();
        state.tick();
        assert_eq!(state.frame, 1);
        state.tick();
        assert_eq!(state.frame, 2);
    }

    #[test]
    fn test_quit() {
        let mut state = AppState::new();
        assert!(!state.should_quit);
        state.quit();
        assert!(state.should_quit);
    }

    #[test]
    fn test_processing() {
        let mut state = AppState::new();
        assert!(!state.is_processing);
        assert!(state.request_start.is_none());

        state.start_processing();
        assert!(state.is_processing);
        assert!(state.request_start.is_some());

        state.stop_processing();
        assert!(!state.is_processing);
        assert!(state.request_start.is_none());
    }

    #[test]
    fn test_history_add() {
        let mut state = AppState::new();
        assert!(state.input_history.is_empty());

        state.add_to_history("first".to_string());
        assert_eq!(state.input_history.len(), 1);

        state.add_to_history("  ".to_string());
        assert_eq!(state.input_history.len(), 1);

        state.add_to_history("first".to_string());
        assert_eq!(state.input_history.len(), 1);

        state.add_to_history("second".to_string());
        assert_eq!(state.input_history.len(), 2);
    }

    #[test]
    fn test_history_navigation() {
        let mut state = AppState::new();
        state.add_to_history("one".to_string());
        state.add_to_history("two".to_string());
        state.add_to_history("three".to_string());

        assert_eq!(state.history_prev(), Some("three".to_string()));
        assert_eq!(state.history_prev(), Some("two".to_string()));
        assert_eq!(state.history_prev(), Some("one".to_string()));
        assert_eq!(state.history_prev(), Some("one".to_string()));

        assert_eq!(state.history_next(), Some("two".to_string()));
        assert_eq!(state.history_next(), Some("three".to_string()));
        assert_eq!(state.history_next(), None);
    }

    #[test]
    fn test_history_limit() {
        let mut state = AppState::new();

        for i in 0..150 {
            state.add_to_history(format!("entry {i}"));
        }

        assert_eq!(state.input_history.len(), 100);

        assert!(!state.input_history.contains(&"entry 0".to_string()));
        assert!(state.input_history.contains(&"entry 149".to_string()));
    }
}
