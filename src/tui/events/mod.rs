mod handler;
mod loops;

pub use handler::TuiToolEventHandler;
pub use loops::{terminal_event_loop, tick_loop};

use crate::core::types::{Message, Usage};
use crate::permission::types::{PermissionRequest, PermissionResponse};
use crate::tools::events::ToolEvent;
use crate::tools::result::ToolResult;
use crossterm::event::KeyEvent;
use std::time::Duration;
use tokio::sync::oneshot;

pub const POLL_TIMEOUT: Duration = Duration::from_millis(100);
pub const TICK_INTERVAL: Duration = Duration::from_millis(16);
pub const SCROLL_DELTA: i16 = 3;

#[derive(Debug)]
pub enum AppEvent {
    Input(KeyEvent),
    Paste(String),
    MouseScroll(i16),
    Resize(u16, u16),
    LLMChunk(String),
    LLMComplete(Message, Usage),
    LLMError(String),
    ToolStarted {
        name: String,
        input: String,
    },
    ToolCompleted {
        name: String,
        result: ToolResult,
    },
    ToolFailed {
        name: String,
        error: String,
    },
    PermissionRequired {
        request: PermissionRequest,
        response_tx: oneshot::Sender<PermissionResponse>,
    },
    FileDiff {
        path: String,
        old_content: String,
        new_content: String,
    },
    Tick,
    ModelChanged {
        provider: String,
        model: String,
    },
    ModelSwitchError(String),
}

impl From<ToolEvent> for AppEvent {
    fn from(event: ToolEvent) -> Self {
        match event {
            ToolEvent::Started { name, input } => Self::ToolStarted { name, input },
            ToolEvent::Completed { name, result } => Self::ToolCompleted { name, result },
            ToolEvent::Failed { name, error } => Self::ToolFailed { name, error },
        }
    }
}
