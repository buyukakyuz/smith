use std::time::Duration;

use crate::tools::{ToolState, ToolType};
use crate::ui::output_widget::MessageLevel;

#[derive(Debug, Clone)]
pub enum ChatMessage {
    User(String),
    Assistant(String),
    System {
        text: String,
        level: MessageLevel,
    },
    StreamingAssistant(String),
    ToolExecution {
        tool_type: ToolType,
        input: String,
        output: Option<String>,
        elapsed: Option<Duration>,
        state: ToolState,
    },
    FileDiff {
        path: String,
        old_content: String,
        new_content: String,
        collapsed: bool,
    },
}
