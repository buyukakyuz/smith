use crate::tui::widgets::{ChatMessage, ScrollState};
use crate::ui::output_widget::MessageLevel;

use super::AppState;

impl AppState {
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

    pub fn add_file_diff(&mut self, path: String, old_content: String, new_content: String) {
        self.messages.push(ChatMessage::FileDiff {
            path,
            old_content,
            new_content,
            collapsed: false,
        });
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
                state: crate::tools::ToolState::InProgress,
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
}
