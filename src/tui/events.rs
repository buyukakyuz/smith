use crate::core::error::Result;
use crate::core::types::Message;
use crate::permission::types::{PermissionRequest, PermissionResponse};
use crate::tools::events::{ToolEvent, ToolEventHandler};
use crate::tools::result::ToolResult;
use crossterm::event::{self, Event as CrosstermEvent, KeyEvent, MouseEventKind};
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::oneshot;

#[derive(Debug)]
pub enum AppEvent {
    Input(KeyEvent),
    Paste(String),
    MouseScroll(i16),
    Resize(u16, u16),
    LLMChunk(String),
    LLMComplete(Message),
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

pub async fn terminal_event_loop(tx: UnboundedSender<AppEvent>) -> Result<()> {
    loop {
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                CrosstermEvent::Key(key) => {
                    if tx.send(AppEvent::Input(key)).is_err() {
                        break;
                    }
                }
                CrosstermEvent::Paste(text) => {
                    if tx.send(AppEvent::Paste(text)).is_err() {
                        break;
                    }
                }
                CrosstermEvent::Resize(w, h) => {
                    if tx.send(AppEvent::Resize(w, h)).is_err() {
                        break;
                    }
                }
                CrosstermEvent::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::ScrollUp => {
                        if tx.send(AppEvent::MouseScroll(-3)).is_err() {
                            break;
                        }
                    }
                    MouseEventKind::ScrollDown => {
                        if tx.send(AppEvent::MouseScroll(3)).is_err() {
                            break;
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }
    Ok(())
}
pub async fn tick_loop(tx: UnboundedSender<AppEvent>) {
    let mut interval = tokio::time::interval(Duration::from_millis(16));
    loop {
        interval.tick().await;
        if tx.send(AppEvent::Tick).is_err() {
            break;
        }
    }
}

pub struct TuiToolEventHandler {
    sender: UnboundedSender<AppEvent>,
}

impl TuiToolEventHandler {
    #[must_use]
    pub const fn new(sender: UnboundedSender<AppEvent>) -> Self {
        Self { sender }
    }
}

impl ToolEventHandler for TuiToolEventHandler {
    fn handle(&self, event: ToolEvent) {
        let app_event = match event {
            ToolEvent::Started { name, input } => AppEvent::ToolStarted { name, input },
            ToolEvent::Completed { name, result } => AppEvent::ToolCompleted { name, result },
            ToolEvent::Failed { name, error } => AppEvent::ToolFailed { name, error },
        };
        let _ = self.sender.send(app_event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn test_tui_tool_event_handler() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let handler = TuiToolEventHandler::new(tx);

        handler.handle(ToolEvent::Started {
            name: "test_tool".to_string(),
            input: "{}".to_string(),
        });

        if let Some(AppEvent::ToolStarted { name, input }) = rx.recv().await {
            assert_eq!(name, "test_tool");
            assert_eq!(input, "{}");
        } else {
            panic!("Expected ToolStarted event");
        }

        let result = ToolResult::success("output");
        handler.handle(ToolEvent::Completed {
            name: "test_tool".to_string(),
            result: result.clone(),
        });

        if let Some(AppEvent::ToolCompleted {
            name,
            result: recv_result,
        }) = rx.recv().await
        {
            assert_eq!(name, "test_tool");
            assert!(recv_result.is_success());
        } else {
            panic!("Expected ToolCompleted event");
        }

        handler.handle(ToolEvent::Failed {
            name: "test_tool".to_string(),
            error: "something went wrong".to_string(),
        });

        if let Some(AppEvent::ToolFailed { name, error }) = rx.recv().await {
            assert_eq!(name, "test_tool");
            assert_eq!(error, "something went wrong");
        } else {
            panic!("Expected ToolFailed event");
        }
    }
}
