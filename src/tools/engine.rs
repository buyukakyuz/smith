use super::context::ToolContext;
use super::events::{ToolEventEmitter, ToolEventHandler};
use super::registry::ToolRegistry;
use super::result::ToolResult;
use super::types::ToolType;
use std::sync::Arc;

pub struct ToolEngine {
    context: ToolContext,
    events: ToolEventEmitter,
}

impl ToolEngine {
    #[must_use]
    pub const fn new(context: ToolContext, events: ToolEventEmitter) -> Self {
        Self { context, events }
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

        let result = registry.execute(tool_name, input).await;

        match result {
            Ok(output) => {
                let (final_output, _truncated) = self.context.truncate_output(output);
                let tool_result = ToolResult::success(final_output);

                self.events.emit_completed(tool_name, tool_result.clone());

                tool_result
            }
            Err(e) => {
                let error_msg = e.to_string();
                let tool_result = self.categorize_error(tool_name, &error_msg);

                self.events.emit_failed(tool_name, &error_msg);

                tool_result
            }
        }
    }

    fn categorize_error(&self, tool_name: &str, error: &str) -> ToolResult {
        let error_lower = error.to_lowercase();

        if error_lower.contains("no such file") || error_lower.contains("not found") {
            return ToolResult::error_with_suggestions(
                error,
                vec![
                    "Verify the file path is correct".to_string(),
                    "Check if the file exists in the expected location".to_string(),
                    format!("Use list_dir to explore the directory"),
                ],
            );
        }

        if error_lower.contains("permission denied") || error_lower.contains("access denied") {
            return ToolResult::error_with_suggestions(
                error,
                vec![
                    "Check file/directory permissions".to_string(),
                    "Ensure you have the necessary access rights".to_string(),
                    "Try using sudo if appropriate (for bash commands)".to_string(),
                ],
            );
        }

        if error_lower.contains("not an absolute path") || error_lower.contains("must be absolute")
        {
            return ToolResult::error_with_suggestions(
                error,
                vec![
                    "Use an absolute path instead of a relative path".to_string(),
                    format!(
                        "Current working directory: {}",
                        self.context.working_dir.display()
                    ),
                ],
            );
        }

        if error_lower.contains("timeout") || error_lower.contains("timed out") {
            return ToolResult::error_with_suggestions(
                error,
                vec![
                    format!(
                        "The operation exceeded the timeout limit ({}s)",
                        self.context.default_timeout_ms / 1000
                    ),
                    "Try breaking the operation into smaller steps".to_string(),
                    "Consider if the operation is hanging or stuck".to_string(),
                ],
            );
        }

        if error_lower.contains("file too large") || error_lower.contains("exceeds limit") {
            return ToolResult::error_with_suggestions(
                error,
                vec![
                    format!(
                        "Maximum file size is {} bytes",
                        self.context.max_output_size
                    ),
                    "Try reading the file in chunks or processing it differently".to_string(),
                ],
            );
        }

        if error_lower.contains("command not found") || error_lower.contains("not recognized") {
            return ToolResult::error_with_suggestions(
                error,
                vec![
                    "Check if the command is installed and in PATH".to_string(),
                    "Verify the command spelling".to_string(),
                    if tool_name == ToolType::Bash.name() {
                        "Use which or whereis to locate the command".to_string()
                    } else {
                        "Try a different approach".to_string()
                    },
                ],
            );
        }

        if error_lower.contains("invalid path") || error_lower.contains("bad path") {
            return ToolResult::error_with_suggestions(
                error,
                vec![
                    "Ensure path is absolute (starts with /)".to_string(),
                    "Check for invalid characters in path".to_string(),
                ],
            );
        }

        if error_lower.contains("binary file") || error_lower.contains("not valid utf-8") {
            return ToolResult::error_with_suggestions(
                error,
                vec![
                    "Cannot read binary files as text".to_string(),
                    "Use appropriate binary analysis tools".to_string(),
                    "Try file command to identify file type".to_string(),
                ],
            );
        }

        if error_lower.contains("not a directory") {
            return ToolResult::error_with_suggestions(
                error,
                vec![
                    "Path exists but is not a directory".to_string(),
                    "Use read_file for files, list_dir for directories".to_string(),
                ],
            );
        }

        if error_lower.contains("invalid pattern")
            || error_lower.contains("invalid glob")
            || error_lower.contains("glob")
        {
            return ToolResult::error_with_suggestions(
                error,
                vec![
                    "Check glob pattern syntax".to_string(),
                    "Examples: *.rs, **/*.txt, src/**/*.{js,ts}".to_string(),
                ],
            );
        }

        if error_lower.contains("regex") || error_lower.contains("invalid regular expression") {
            return ToolResult::error_with_suggestions(
                error,
                vec![
                    "Check regex pattern syntax".to_string(),
                    r"Escape special characters: . * + ? [ ] ( ) { } ^ $ | \".to_string(),
                    "Test regex at regex101.com".to_string(),
                ],
            );
        }

        if error_lower.contains("parent directory") {
            return ToolResult::error_with_suggestions(
                error,
                vec![
                    "Parent directory does not exist".to_string(),
                    "Use create_dirs: true to create parent directories".to_string(),
                    "Or create the directory first using bash mkdir -p".to_string(),
                ],
            );
        }

        ToolResult::error(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::error::{AgentError, Result};
    use crate::tools::{ToolRegistry, TypedTool};
    use async_trait::async_trait;
    use schemars::JsonSchema;
    use serde::Deserialize;
    use std::path::PathBuf;
    use std::sync::Arc;

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
