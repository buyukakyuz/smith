use async_trait::async_trait;
use schemars::JsonSchema;

use crate::core::error::Result;

pub mod context;
pub mod engine;
pub mod events;
pub mod executor;
pub mod fs;
pub mod registry;
pub mod result;
pub mod shell;
pub mod types;

pub use context::ToolContext;
pub use engine::ToolEngine;
pub use events::{ToolEventEmitter, ToolEventHandler};
pub use executor::ToolExecutor;
pub use fs::{GlobTool, GrepTool, ListDirTool, ReadFileTool, UpdateFileTool, WriteFileTool};
pub use registry::ToolRegistry;
pub use shell::BashTool;
pub use types::{ToolState, ToolType};

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> serde_json::Value;
    async fn execute(&self, input: serde_json::Value) -> Result<String>;
}

#[async_trait]
pub trait TypedTool: Send + Sync {
    type Input: JsonSchema + for<'de> serde::Deserialize<'de>;

    fn name(&self) -> &str;

    fn description(&self) -> &str;

    async fn execute_typed(&self, input: Self::Input) -> Result<String>;
}

#[async_trait]
impl<T: TypedTool> Tool for T {
    fn name(&self) -> &str {
        TypedTool::name(self)
    }

    fn description(&self) -> &str {
        TypedTool::description(self)
    }

    fn input_schema(&self) -> serde_json::Value {
        let mut schema = serde_json::to_value(schemars::schema_for!(T::Input))
            .unwrap_or_else(|_| serde_json::json!({}));

        if let Some(obj) = schema.as_object_mut()
            && obj.get("type").and_then(|t| t.as_str()) == Some("object")
            && !obj.contains_key("properties")
        {
            obj.insert("properties".to_string(), serde_json::json!({}));
        }
        schema
    }

    async fn execute(&self, input: serde_json::Value) -> Result<String> {
        let typed_input: T::Input = serde_json::from_value(input)?;
        self.execute_typed(typed_input).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use schemars::JsonSchema;
    use serde::Deserialize;

    #[derive(Debug, Deserialize, JsonSchema)]
    struct TestInput {
        message: String,
        count: u32,
    }

    struct EchoTool;

    #[async_trait]
    impl TypedTool for EchoTool {
        type Input = TestInput;

        fn name(&self) -> &'static str {
            "echo"
        }

        fn description(&self) -> &'static str {
            "Echoes the input message multiple times"
        }

        async fn execute_typed(&self, input: Self::Input) -> Result<String> {
            Ok(input.message.repeat(input.count as usize))
        }
    }

    #[tokio::test]
    async fn test_typed_tool_name_and_description() {
        let tool = EchoTool;
        assert_eq!(Tool::name(&tool), "echo");
        assert_eq!(
            Tool::description(&tool),
            "Echoes the input message multiple times"
        );
    }

    #[tokio::test]
    async fn test_typed_tool_schema_generation() {
        let tool = EchoTool;
        let schema = tool.input_schema();

        assert!(schema.is_object());

        let properties = schema.get("properties");
        assert!(properties.is_some());
    }

    #[tokio::test]
    async fn test_typed_tool_execution() {
        let tool = EchoTool;
        let input = serde_json::json!({
            "message": "Hi",
            "count": 3
        });

        let result = tool.execute(input).await.unwrap();
        assert_eq!(result, "HiHiHi");
    }

    #[tokio::test]
    async fn test_typed_tool_invalid_input() {
        let tool = EchoTool;
        let input = serde_json::json!({
            "invalid_field": "value"
        });

        let result = tool.execute(input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_tool_trait_object_safety() {
        let tool: Box<dyn Tool> = Box::new(EchoTool);
        assert_eq!(tool.name(), "echo");

        let input = serde_json::json!({
            "message": "Test",
            "count": 2
        });

        let result = tool.execute(input).await.unwrap();
        assert_eq!(result, "TestTest");
    }
}
