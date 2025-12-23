use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use std::time::Instant;

use crate::core::error::{AgentError, Result};
use crate::core::metadata;
use crate::tools::ToolType;
use crate::tools::TypedTool;

use super::constants::MAX_WRITE_SIZE;
use super::validate_absolute_path;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct WriteFileInput {
    pub path: String,
    pub content: String,
    #[serde(default = "default_create_dirs")]
    pub create_dirs: bool,
}

const fn default_create_dirs() -> bool {
    true
}

pub struct WriteFileTool;

impl WriteFileTool {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for WriteFileTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TypedTool for WriteFileTool {
    type Input = WriteFileInput;

    fn name(&self) -> &'static str {
        "write_file"
    }

    fn description(&self) -> &'static str {
        "Write content to a file atomically. Creates parent directories by default. The path must be absolute."
    }

    async fn execute_typed(&self, input: Self::Input) -> Result<String> {
        let start_time = Instant::now();
        let path = validate_absolute_path(&input.path, ToolType::WriteFile)?;

        if input.content.len() > MAX_WRITE_SIZE {
            return Err(AgentError::InvalidToolInput {
                tool: ToolType::WriteFile.name().to_string(),
                reason: format!(
                    "Content too large: {} bytes (max: {} bytes)",
                    input.content.len(),
                    MAX_WRITE_SIZE
                ),
            });
        }

        let old_content = if path.exists() {
            std::fs::read_to_string(&path).ok()
        } else {
            None
        };

        if input.create_dirs {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
        } else if let Some(parent) = path.parent()
            && !parent.exists()
        {
            return Err(AgentError::InvalidToolInput {
                tool: ToolType::WriteFile.name().to_string(),
                reason: format!(
                    "Parent directory does not exist: {}. Use create_dirs: true to create it.",
                    parent.display()
                ),
            });
        }

        let temp_path = path.with_extension("tmp");

        std::fs::write(&temp_path, &input.content).inspect_err(|_e| {
            let _ = std::fs::remove_file(&temp_path);
        })?;

        std::fs::rename(&temp_path, &path).inspect_err(|_e| {
            let _ = std::fs::remove_file(&temp_path);
        })?;

        let metadata = std::fs::metadata(&path)?;
        let file_size = metadata.len();
        let line_count = input.content.lines().count();

        let _execution_time = start_time.elapsed();

        let output = format!(
            "Wrote {} bytes ({} lines) to {}",
            file_size,
            line_count,
            path.display()
        );

        let output_with_metadata = if old_content.is_some() || !input.content.is_empty() {
            let metadata_json = json!({
                "diff_metadata": {
                    "path": path.to_string_lossy(),
                    "old_content": old_content.unwrap_or_default(),
                    "new_content": input.content,
                }
            });
            format!("{output}\n\n{}", metadata::wrap(&metadata_json.to_string()))
        } else {
            output
        };

        Ok(output_with_metadata)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::Tool;

    #[test]
    fn test_write_file_tool_name() {
        let tool = WriteFileTool::new();
        assert_eq!(Tool::name(&tool), "write_file");
    }

    #[tokio::test]
    async fn test_write_file_basic() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        let tool = WriteFileTool::new();
        let input = WriteFileInput {
            path: file_path.to_str().unwrap().to_string(),
            content: "Hello, World!".to_string(),
            create_dirs: true,
        };

        let result = tool.execute_typed(input).await.unwrap();
        assert!(result.contains("Wrote"));
        assert!(result.contains("bytes"));

        let contents = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(contents, "Hello, World!");
    }

    #[tokio::test]
    async fn test_write_file_multiline() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        let tool = WriteFileTool::new();
        let input = WriteFileInput {
            path: file_path.to_str().unwrap().to_string(),
            content: "Line 1\nLine 2\nLine 3".to_string(),
            create_dirs: true,
        };

        let result = tool.execute_typed(input).await.unwrap();
        assert!(result.contains("3 lines"));

        let contents = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(contents, "Line 1\nLine 2\nLine 3");
    }

    #[tokio::test]
    async fn test_write_file_creates_parent_dirs() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("subdir").join("test.txt");

        let tool = WriteFileTool::new();
        let input = WriteFileInput {
            path: file_path.to_str().unwrap().to_string(),
            content: "Test".to_string(),
            create_dirs: true,
        };

        let result = tool.execute_typed(input).await.unwrap();
        assert!(result.contains("Wrote"));

        assert!(file_path.exists());
    }

    #[tokio::test]
    async fn test_write_file_fails_without_create_dirs() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("nonexistent").join("test.txt");

        let tool = WriteFileTool::new();
        let input = WriteFileInput {
            path: file_path.to_str().unwrap().to_string(),
            content: "Test".to_string(),
            create_dirs: false,
        };

        let result = tool.execute_typed(input).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Parent directory does not exist")
        );
    }

    #[tokio::test]
    async fn test_write_file_overwrites() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        std::fs::write(&file_path, "Initial").unwrap();

        let tool = WriteFileTool::new();
        let input = WriteFileInput {
            path: file_path.to_str().unwrap().to_string(),
            content: "Updated".to_string(),
            create_dirs: true,
        };

        tool.execute_typed(input).await.unwrap();

        let contents = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(contents, "Updated");
    }
}
