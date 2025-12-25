use std::sync::Arc;
use tokio::sync::RwLock;

use crate::core::error::Result;
use crate::core::types::{ContentBlock, Message, Role};
use crate::permission::{
    PermissionCheckResult, PermissionManager, PermissionRequest, PermissionType,
};
use crate::tools::{ToolEngine, ToolRegistry, ToolType};

pub struct ToolExecutor<'a> {
    tools: &'a ToolRegistry,
    permission_manager: Option<&'a Arc<PermissionManager>>,
    engine: &'a ToolEngine,
    parallel_lock: Arc<RwLock<()>>,
}

impl<'a> ToolExecutor<'a> {
    #[must_use]
    pub fn new(
        tools: &'a ToolRegistry,
        permission_manager: Option<&'a Arc<PermissionManager>>,
        engine: &'a ToolEngine,
    ) -> Self {
        Self {
            tools,
            permission_manager,
            engine,
            parallel_lock: Arc::new(RwLock::new(())),
        }
    }

    fn check_permission(
        &self,
        tool_type: &ToolType,
        tool_input: &serde_json::Value,
    ) -> Result<Option<String>> {
        let Some(manager) = self.permission_manager.as_ref() else {
            return Ok(None);
        };

        let (perm_type, target) = match tool_type {
            ToolType::Bash => {
                let command = tool_input
                    .get("command")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown command");
                (PermissionType::CommandExecute, command.to_string())
            }
            ToolType::WriteFile | ToolType::UpdateFile => {
                let path = tool_input
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown path");
                (PermissionType::FileWrite, path.to_string())
            }
            ToolType::ReadFile | ToolType::ListDir | ToolType::Glob | ToolType::Grep => {
                let path = tool_input
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or(".");
                (PermissionType::FileRead, path.to_string())
            }
            _ => {
                return Ok(None);
            }
        };

        let request = PermissionRequest::new(perm_type, target).with_context(format!(
            "Tool '{}' requested by AI assistant",
            tool_type.name()
        ));

        match manager.check_permission(&request)? {
            PermissionCheckResult::Allowed => Ok(None),
            PermissionCheckResult::DeniedWithFeedback(feedback) => Ok(Some(feedback)),
        }
    }

    pub async fn execute_tools(&self, assistant_message: &Message) -> Vec<Message> {
        let mut results = Vec::new();

        for content_block in &assistant_message.content {
            if let ContentBlock::ToolUse {
                id, name, input, ..
            } = content_block
            {
                let tool_type = ToolType::from_name(name);

                let permission_denial = match self.check_permission(&tool_type, input) {
                    Ok(denial_feedback) => denial_feedback,
                    Err(e) => {
                        let error_msg = format!("Permission check failed: {e}");
                        results.push(Message::new(
                            Role::Tool,
                            vec![ContentBlock::tool_error(id, error_msg)],
                        ));
                        continue;
                    }
                };

                let tool_result = if let Some(user_feedback) = permission_denial {
                    let error_message =
                        format!("Operation blocked by user. User feedback: {user_feedback}");
                    ContentBlock::tool_error(id, error_message)
                } else {
                    let result = if tool_type.is_read_only() {
                        let _read_guard = self.parallel_lock.read().await;
                        self.engine.execute(self.tools, name, input.clone()).await
                    } else {
                        let _write_guard = self.parallel_lock.write().await;
                        self.engine.execute(self.tools, name, input.clone()).await
                    };

                    if result.is_success() {
                        ContentBlock::tool_result(id, result.to_llm_string())
                    } else {
                        ContentBlock::tool_error(id, result.to_llm_string())
                    }
                };

                results.push(Message::new(Role::Tool, vec![tool_result]));
            }
        }

        results
    }
}
