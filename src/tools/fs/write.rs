use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use std::path::{Path, PathBuf};

use crate::core::error::{AgentError, Result};
use crate::core::metadata;
use crate::tools::{ToolType, TypedTool};

use super::constants::MAX_WRITE_SIZE;
use super::{atomic_write, validate_absolute_path};
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
struct WriteResult {
    path: PathBuf,
    bytes_written: usize,
    line_count: usize,
    old_content: Option<String>,
    new_content: String,
}
fn validate_content_size(content: &str) -> Result<()> {
    if content.len() > MAX_WRITE_SIZE {
        return Err(AgentError::InvalidToolInput {
            tool: ToolType::WriteFile.name().to_string(),
            reason: format!(
                "Content too large: {} bytes (max: {MAX_WRITE_SIZE} bytes)",
                content.len()
            ),
        });
    }
    Ok(())
}

fn ensure_parent_directory(path: &Path, create: bool) -> Result<()> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };

    if parent.exists() {
        return Ok(());
    }

    if create {
        std::fs::create_dir_all(parent)?;
    } else {
        return Err(AgentError::InvalidToolInput {
            tool: ToolType::WriteFile.name().to_string(),
            reason: format!(
                "Parent directory does not exist: {}. Use create_dirs: true to create it.",
                parent.display()
            ),
        });
    }

    Ok(())
}

fn execute_write(path: &Path, content: String, create_dirs: bool) -> Result<WriteResult> {
    validate_content_size(&content)?;
    ensure_parent_directory(path, create_dirs)?;

    let old_content = path
        .is_file()
        .then(|| std::fs::read_to_string(path).ok())
        .flatten();

    atomic_write(path, &content)?;

    Ok(WriteResult {
        path: path.to_path_buf(),
        bytes_written: content.len(),
        line_count: content.lines().count(),
        old_content,
        new_content: content,
    })
}

fn format_output(result: WriteResult) -> String {
    let summary = format!(
        "Wrote {} bytes ({} lines) to {}",
        result.bytes_written,
        result.line_count,
        result.path.display()
    );

    let metadata_json = json!({
        "diff_metadata": {
            "path": result.path.to_string_lossy(),
            "old_content": result.old_content.unwrap_or_default(),
            "new_content": result.new_content,
        }
    });

    format!(
        "{summary}\n\n{}",
        metadata::wrap(&metadata_json.to_string())
    )
}
#[derive(Default)]
pub struct WriteFileTool;

impl WriteFileTool {
    #[must_use]
    pub const fn new() -> Self {
        Self
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
        let path = validate_absolute_path(&input.path, &ToolType::WriteFile)?;
        let result = execute_write(&path, input.content, input.create_dirs)?;
        Ok(format_output(result))
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
