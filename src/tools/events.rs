use super::result::ToolResult;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum ToolEvent {
    Started { name: String, input: String },
    Completed { name: String, result: ToolResult },
    Failed { name: String, error: String },
}

pub trait ToolEventHandler: Send + Sync {
    fn handle(&self, event: ToolEvent);
}

pub struct ToolEventEmitter {
    handlers: Vec<Arc<dyn ToolEventHandler>>,
}

impl ToolEventEmitter {
    #[must_use]
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
        }
    }

    pub fn add_handler(&mut self, handler: Arc<dyn ToolEventHandler>) {
        self.handlers.push(handler);
    }

    pub fn emit(&self, event: ToolEvent) {
        for handler in &self.handlers {
            handler.handle(event.clone());
        }
    }

    pub fn emit_started(&self, name: impl Into<String>, input: impl Into<String>) {
        self.emit(ToolEvent::Started {
            name: name.into(),
            input: input.into(),
        });
    }

    pub fn emit_completed(&self, name: impl Into<String>, result: ToolResult) {
        self.emit(ToolEvent::Completed {
            name: name.into(),
            result,
        });
    }

    pub fn emit_failed(&self, name: impl Into<String>, error: impl Into<String>) {
        self.emit(ToolEvent::Failed {
            name: name.into(),
            error: error.into(),
        });
    }
}

impl Default for ToolEventEmitter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct TestHandler {
        events: Arc<Mutex<Vec<ToolEvent>>>,
    }

    impl ToolEventHandler for TestHandler {
        fn handle(&self, event: ToolEvent) {
            self.events.lock().unwrap().push(event);
        }
    }

    #[test]
    fn test_event_emitter() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let handler = Arc::new(TestHandler {
            events: Arc::clone(&events),
        });

        let mut emitter = ToolEventEmitter::new();
        emitter.add_handler(handler);

        emitter.emit_started("test_tool", "{}");

        let result = ToolResult::success("output");
        emitter.emit_completed("test_tool", result);

        let captured_events = events.lock().unwrap();
        assert_eq!(captured_events.len(), 2);

        match &captured_events[0] {
            ToolEvent::Started { name, input } => {
                assert_eq!(name, "test_tool");
                assert_eq!(input, "{}");
            }
            _ => panic!("Expected Started event"),
        }

        match &captured_events[1] {
            ToolEvent::Completed { name, result } => {
                assert_eq!(name, "test_tool");
                assert!(result.is_success());
            }
            _ => panic!("Expected Completed event"),
        }
    }

    #[test]
    fn test_failed_event() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let handler = Arc::new(TestHandler {
            events: Arc::clone(&events),
        });

        let mut emitter = ToolEventEmitter::new();
        emitter.add_handler(handler);

        emitter.emit_failed("test_tool", "something went wrong");

        let captured_events = events.lock().unwrap();
        assert_eq!(captured_events.len(), 1);

        match &captured_events[0] {
            ToolEvent::Failed { name, error } => {
                assert_eq!(name, "test_tool");
                assert_eq!(error, "something went wrong");
            }
            _ => panic!("Expected Failed event"),
        }
    }
}
