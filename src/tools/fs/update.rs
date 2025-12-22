use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Instant;

use crate::core::error::{AgentError, Result};
use crate::core::metadata;
use crate::tools::TypedTool;

use super::{validate_absolute_path, validate_file_size, validate_path_exists};
use crate::tools::ToolType;

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct UpdateFileInput {
    pub path: String,
    pub old_string: String,
    pub new_string: String,
    #[serde(default)]
    pub replace_all: bool,
}

pub struct UpdateFileTool;

impl UpdateFileTool {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for UpdateFileTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TypedTool for UpdateFileTool {
    type Input = UpdateFileInput;

    fn name(&self) -> &'static str {
        "update_file"
    }

    fn description(&self) -> &'static str {
        "Update a file by replacing exact text matches. Finds old_string and replaces it with new_string. \
         Use replace_all=true to replace all occurrences. The path must be absolute."
    }

    async fn execute_typed(&self, input: Self::Input) -> Result<String> {
        let start_time = Instant::now();
        let path = validate_absolute_path(&input.path, ToolType::UpdateFile)?;

        validate_path_exists(&path, ToolType::UpdateFile)?;
        validate_file_size(&path, ToolType::UpdateFile)?;

        let old_content = std::fs::read_to_string(&path)?;

        if !old_content.contains(&input.old_string) {
            return Err(AgentError::InvalidToolInput {
                tool: ToolType::UpdateFile.name().to_string(),
                reason: format!(
                    "Could not find old_string in file. Make sure it matches exactly:\n{}",
                    input.old_string
                ),
            });
        }

        if !input.replace_all {
            let match_count = old_content.matches(&input.old_string).count();
            if match_count > 1 {
                return Err(AgentError::InvalidToolInput {
                    tool: ToolType::UpdateFile.name().to_string(),
                    reason: format!(
                        "Found {match_count} occurrences of old_string. Use replace_all=true to replace all, or provide a longer, unique string."
                    ),
                });
            }
        }

        let new_content = if input.replace_all {
            old_content.replace(&input.old_string, &input.new_string)
        } else {
            old_content.replacen(&input.old_string, &input.new_string, 1)
        };

        if old_content == new_content {
            return Ok(format!(
                "No changes made to {} (old_string and new_string are identical)",
                path.display()
            ));
        }

        let temp_path = path.with_extension("tmp");

        std::fs::write(&temp_path, &new_content).inspect_err(|_e| {
            let _ = std::fs::remove_file(&temp_path);
        })?;

        std::fs::rename(&temp_path, &path).inspect_err(|_e| {
            let _ = std::fs::remove_file(&temp_path);
        })?;

        let _execution_time = start_time.elapsed();

        let occurrences = if input.replace_all {
            old_content.matches(&input.old_string).count()
        } else {
            1
        };

        let output = format!(
            "Updated {} ({} occurrence{})",
            path.display(),
            occurrences,
            if occurrences == 1 { "" } else { "s" }
        );

        let metadata_json = json!({
            "diff_metadata": {
                "path": path.to_string_lossy(),
                "old_content": old_content,
                "new_content": new_content,
            }
        });
        let output_with_metadata =
            format!("{output}\n\n{}", metadata::wrap(&metadata_json.to_string()));

        Ok(output_with_metadata)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::Tool;

    #[test]
    fn test_update_file_tool_name() {
        let tool = UpdateFileTool::new();
        assert_eq!(Tool::name(&tool), "update_file");
    }

    #[tokio::test]
    async fn test_update_file_basic() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        std::fs::write(&file_path, "Hello, World!").unwrap();

        let tool = UpdateFileTool::new();
        let input = UpdateFileInput {
            path: file_path.to_str().unwrap().to_string(),
            old_string: "World".to_string(),
            new_string: "Rust".to_string(),
            replace_all: false,
        };

        let result = tool.execute_typed(input).await.unwrap();
        assert!(result.contains("Updated"));
        assert!(result.contains("1 occurrence"));

        let contents = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(contents, "Hello, Rust!");
    }

    #[tokio::test]
    async fn test_update_file_replace_all() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        std::fs::write(&file_path, "foo bar foo baz foo").unwrap();

        let tool = UpdateFileTool::new();
        let input = UpdateFileInput {
            path: file_path.to_str().unwrap().to_string(),
            old_string: "foo".to_string(),
            new_string: "qux".to_string(),
            replace_all: true,
        };

        let result = tool.execute_typed(input).await.unwrap();
        assert!(result.contains("3 occurrences"));

        let contents = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(contents, "qux bar qux baz qux");
    }

    #[tokio::test]
    async fn test_update_file_not_found() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("nonexistent.txt");

        let tool = UpdateFileTool::new();
        let input = UpdateFileInput {
            path: file_path.to_str().unwrap().to_string(),
            old_string: "foo".to_string(),
            new_string: "bar".to_string(),
            replace_all: false,
        };

        let result = tool.execute_typed(input).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not exist"));
    }

    #[tokio::test]
    async fn test_update_file_old_string_not_found() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        std::fs::write(&file_path, "Hello, World!").unwrap();

        let tool = UpdateFileTool::new();
        let input = UpdateFileInput {
            path: file_path.to_str().unwrap().to_string(),
            old_string: "Goodbye".to_string(),
            new_string: "Hello".to_string(),
            replace_all: false,
        };

        let result = tool.execute_typed(input).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Could not find old_string")
        );
    }

    #[tokio::test]
    async fn test_update_file_ambiguous_match() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        std::fs::write(&file_path, "foo bar foo baz").unwrap();

        let tool = UpdateFileTool::new();
        let input = UpdateFileInput {
            path: file_path.to_str().unwrap().to_string(),
            old_string: "foo".to_string(),
            new_string: "qux".to_string(),
            replace_all: false,
        };

        let result = tool.execute_typed(input).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Found 2 occurrences")
        );
    }

    #[tokio::test]
    async fn test_update_file_multiline() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        let initial = "Line 1\nLine 2\nLine 3\n";
        std::fs::write(&file_path, initial).unwrap();

        let tool = UpdateFileTool::new();
        let input = UpdateFileInput {
            path: file_path.to_str().unwrap().to_string(),
            old_string: "Line 2\n".to_string(),
            new_string: "Modified Line 2\n".to_string(),
            replace_all: false,
        };

        let result = tool.execute_typed(input).await.unwrap();
        assert!(result.contains("Updated"));

        let contents = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(contents, "Line 1\nModified Line 2\nLine 3\n");
    }

    #[tokio::test]
    async fn test_update_file_no_change() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        std::fs::write(&file_path, "Hello, World!").unwrap();

        let tool = UpdateFileTool::new();
        let input = UpdateFileInput {
            path: file_path.to_str().unwrap().to_string(),
            old_string: "World".to_string(),
            new_string: "World".to_string(),
            replace_all: false,
        };

        let result = tool.execute_typed(input).await.unwrap();
        assert!(result.contains("No changes made"));
    }
}
