use std::time::Instant;

use crate::core::metadata;
use crate::tools::result::ToolResult;
use crate::tools::{ToolState, ToolType};
use crate::tui::widgets::ChatMessage;

use super::AppState;

#[derive(Debug, Clone)]
pub struct ToolExecution {
    pub tool_type: ToolType,
    pub input: String,
    pub started_at: Instant,
}

impl AppState {
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

    pub fn complete_tool(&mut self, name: &str, result: &ToolResult) {
        let Some(execution) = self.active_tools.remove(name) else {
            return;
        };

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

    #[allow(clippy::needless_pass_by_value)]
    pub fn fail_tool(&mut self, name: &str, error: String) {
        let Some(execution) = self.active_tools.remove(name) else {
            return;
        };

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
