use super::context::ToolContext;
use super::error_hints::{ErrorContext, ErrorHintMatcher};
use super::events::{ToolEventEmitter, ToolEventHandler};
use super::registry::ToolRegistry;
use super::result::ToolResult;
use std::sync::Arc;

pub struct ToolEngine {
    context: ToolContext,
    events: ToolEventEmitter,
    error_hints: ErrorHintMatcher,
}

impl ToolEngine {
    #[must_use]
    pub fn new(context: ToolContext, events: ToolEventEmitter) -> Self {
        Self {
            context,
            events,
            error_hints: ErrorHintMatcher::new(),
        }
    }

    pub fn register_handler(&mut self, handler: Arc<dyn ToolEventHandler>) {
        self.events.add_handler(handler);
    }

    pub async fn execute(
        &self,
        registry: &ToolRegistry,
        tool_name: &str,
        input: serde_json::Value,
    ) -> ToolResult {
        let input_str = serde_json::to_string(&input).unwrap_or_else(|_| "{}".to_string());
        self.events.emit_started(tool_name, &input_str);

        match registry.execute(tool_name, input).await {
            Ok(output) => {
                let (final_output, _truncated) = self.context.truncate_output(output);
                let result = ToolResult::success(final_output);
                self.events.emit_completed(tool_name, result.clone());
                result
            }
            Err(e) => {
                let error_msg = e.to_string();
                let ctx = ErrorContext {
                    tool_name,
                    working_dir: &self.context.working_dir,
                    default_timeout_ms: self.context.default_timeout_ms,
                    max_output_size: self.context.max_output_size,
                };
                let result = self.error_hints.categorize(&ctx, &error_msg);
                self.events.emit_failed(tool_name, &error_msg);
                result
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::error::{AgentError, Result};
    use crate::tools::TypedTool;
    use async_trait::async_trait;
    use schemars::JsonSchema;
    use serde::Deserialize;
    use std::path::PathBuf;

    #[derive(Debug, Deserialize, JsonSchema)]
    struct TestInput {
        value: String,
    }

    struct SuccessTool;

    #[async_trait]
    impl TypedTool for SuccessTool {
        type Input = TestInput;

        fn name(&self) -> &'static str {
            "success_tool"
        }

        fn description(&self) -> &'static str {
            "A tool that always succeeds"
        }

        async fn execute_typed(&self, input: Self::Input) -> Result<String> {
            Ok(format!("Success: {}", input.value))
        }
    }

    struct ErrorTool;

    #[async_trait]
    impl TypedTool for ErrorTool {
        type Input = TestInput;

        fn name(&self) -> &'static str {
            "error_tool"
        }

        fn description(&self) -> &'static str {
            "A tool that always fails"
        }

        async fn execute_typed(&self, _input: Self::Input) -> Result<String> {
            Err(AgentError::ToolExecution(
                "No such file or directory".into(),
            ))
        }
    }

    #[tokio::test]
    async fn test_engine_success() {
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(SuccessTool));

        let context = ToolContext::with_working_dir(PathBuf::from("/tmp"));
        let events = ToolEventEmitter::new();
        let engine = ToolEngine::new(context, events);

        let input = serde_json::json!({"value": "test"});
        let result = engine.execute(&registry, "success_tool", input).await;

        assert!(result.is_success());
        assert_eq!(result.output(), Some("Success: test"));
    }

    #[tokio::test]
    async fn test_engine_error_with_suggestions() {
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(ErrorTool));

        let context = ToolContext::with_working_dir(PathBuf::from("/tmp"));
        let events = ToolEventEmitter::new();
        let engine = ToolEngine::new(context, events);

        let input = serde_json::json!({"value": "test"});
        let result = engine.execute(&registry, "error_tool", input).await;

        assert!(!result.is_success());

        let llm_output = result.to_llm_string();
        assert!(llm_output.contains("Error"));
        assert!(llm_output.contains("file path") || llm_output.contains("location"));
    }

    #[tokio::test]
    async fn test_engine_truncation() {
        struct LargeOutputTool;

        #[async_trait]
        impl TypedTool for LargeOutputTool {
            type Input = TestInput;

            fn name(&self) -> &'static str {
                "large_output"
            }

            fn description(&self) -> &'static str {
                "A tool with large output"
            }

            async fn execute_typed(&self, _input: Self::Input) -> Result<String> {
                Ok("x".repeat(1000))
            }
        }

        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(LargeOutputTool));

        let mut context = ToolContext::with_working_dir(PathBuf::from("/tmp"));
        context.max_output_size = 100;
        let events = ToolEventEmitter::new();
        let engine = ToolEngine::new(context, events);

        let input = serde_json::json!({"value": "test"});
        let result = engine.execute(&registry, "large_output", input).await;

        assert!(result.is_success());
        assert!(result.output().unwrap().contains("truncated"));
    }
}
