use super::AppEvent;
use crate::tools::events::{ToolEvent, ToolEventHandler};
use tokio::sync::mpsc::UnboundedSender;

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
        let _ = self.sender.send(event.into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::result::ToolResult;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn test_tool_started_event() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let handler = TuiToolEventHandler::new(tx);

        handler.handle(ToolEvent::Started {
            name: "test_tool".to_string(),
            input: "{}".to_string(),
        });

        let event = rx.recv().await.expect("Expected event");
        assert!(
            matches!(event, AppEvent::ToolStarted { name, input } if name == "test_tool" && input == "{}")
        );
    }

    #[tokio::test]
    async fn test_tool_completed_event() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let handler = TuiToolEventHandler::new(tx);

        handler.handle(ToolEvent::Completed {
            name: "test_tool".to_string(),
            result: ToolResult::success("output"),
        });

        let event = rx.recv().await.expect("Expected event");
        assert!(
            matches!(event, AppEvent::ToolCompleted { name, result } if name == "test_tool" && result.is_success())
        );
    }

    #[tokio::test]
    async fn test_tool_failed_event() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let handler = TuiToolEventHandler::new(tx);

        handler.handle(ToolEvent::Failed {
            name: "test_tool".to_string(),
            error: "something went wrong".to_string(),
        });

        let event = rx.recv().await.expect("Expected event");
        assert!(
            matches!(event, AppEvent::ToolFailed { name, error } if name == "test_tool" && error == "something went wrong")
        );
    }
}
