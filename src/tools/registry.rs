use std::collections::HashMap;
use std::sync::Arc;

use crate::core::error::{AgentError, Result};
use crate::core::types::ToolDefinition;

use super::Tool;

#[derive(Clone, Default)]
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        let name = tool.name().to_string();
        self.tools.insert(name, tool);
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    pub async fn execute(&self, name: &str, input: serde_json::Value) -> Result<String> {
        let tool = self
            .get(name)
            .ok_or_else(|| AgentError::ToolNotFound(name.to_string()))?;

        tool.execute(input)
            .await
            .map_err(|e| AgentError::ToolExecution(format!("Tool '{name}' failed: {e}")))
    }

    #[must_use]
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools
            .values()
            .map(|tool| ToolDefinition::new(tool.name(), tool.description(), tool.input_schema()))
            .collect()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    #[must_use]
    pub fn names(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }

    #[cfg(test)]
    pub fn has(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    #[cfg(test)]
    pub fn clear(&mut self) {
        self.tools.clear();
    }

    #[cfg(test)]
    pub fn remove(&mut self, name: &str) -> bool {
        self.tools.remove(name).is_some()
    }
}

impl std::fmt::Debug for ToolRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolRegistry")
            .field("tool_count", &self.len())
            .field("tools", &self.names())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::TypedTool;
    use async_trait::async_trait;
    use schemars::JsonSchema;
    use serde::Deserialize;

    #[derive(Debug, Deserialize, JsonSchema)]
    struct AddInput {
        a: i32,
        b: i32,
    }

    struct AddTool;

    #[async_trait]
    impl TypedTool for AddTool {
        type Input = AddInput;

        fn name(&self) -> &'static str {
            "add"
        }

        fn description(&self) -> &'static str {
            "Adds two numbers"
        }

        async fn execute_typed(&self, input: Self::Input) -> Result<String> {
            Ok((input.a + input.b).to_string())
        }
    }

    #[derive(Debug, Deserialize, JsonSchema)]
    struct MultiplyInput {
        a: i32,
        b: i32,
    }

    struct MultiplyTool;

    #[async_trait]
    impl TypedTool for MultiplyTool {
        type Input = MultiplyInput;

        fn name(&self) -> &'static str {
            "multiply"
        }

        fn description(&self) -> &'static str {
            "Multiplies two numbers"
        }

        async fn execute_typed(&self, input: Self::Input) -> Result<String> {
            Ok((input.a * input.b).to_string())
        }
    }

    #[test]
    fn test_registry_new() {
        let registry = ToolRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_registry_register_and_get() {
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(AddTool));

        assert_eq!(registry.len(), 1);
        assert!(registry.has("add"));
        assert!(!registry.is_empty());

        let tool = registry.get("add");
        assert!(tool.is_some());
        assert_eq!(tool.unwrap().name(), "add");
    }

    #[test]
    fn test_registry_multiple_tools() {
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(AddTool));
        registry.register(Arc::new(MultiplyTool));

        assert_eq!(registry.len(), 2);
        assert!(registry.has("add"));
        assert!(registry.has("multiply"));

        let names = registry.names();
        assert!(names.contains(&"add".to_string()));
        assert!(names.contains(&"multiply".to_string()));
    }

    #[tokio::test]
    async fn test_registry_execute() {
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(AddTool));

        let input = serde_json::json!({"a": 5, "b": 3});
        let result = registry.execute("add", input).await.unwrap();
        assert_eq!(result, "8");
    }

    #[tokio::test]
    async fn test_registry_execute_tool_not_found() {
        let registry = ToolRegistry::new();
        let input = serde_json::json!({"a": 5, "b": 3});
        let result = registry.execute("nonexistent", input).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AgentError::ToolNotFound(_)));
    }

    #[test]
    fn test_registry_definitions() {
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(AddTool));
        registry.register(Arc::new(MultiplyTool));

        let definitions = registry.definitions();
        assert_eq!(definitions.len(), 2);

        let add_def = definitions.iter().find(|d| d.name == "add").unwrap();
        assert_eq!(add_def.description, "Adds two numbers");
        assert!(add_def.input_schema.is_object());
    }

    #[test]
    fn test_registry_clear() {
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(AddTool));
        registry.register(Arc::new(MultiplyTool));

        assert_eq!(registry.len(), 2);

        registry.clear();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_registry_remove() {
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(AddTool));
        registry.register(Arc::new(MultiplyTool));

        assert!(registry.remove("add"));
        assert_eq!(registry.len(), 1);
        assert!(!registry.has("add"));
        assert!(registry.has("multiply"));

        assert!(!registry.remove("nonexistent"));
    }

    #[test]
    fn test_registry_debug() {
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(AddTool));

        let debug_str = format!("{registry:?}");
        assert!(debug_str.contains("ToolRegistry"));
        assert!(debug_str.contains("add"));
    }

    #[test]
    fn test_registry_clone() {
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(AddTool));

        let cloned = registry.clone();
        assert_eq!(cloned.len(), registry.len());
        assert!(cloned.has("add"));
    }
}
