use async_trait::async_trait;
use content_inspector::ContentType;
use schemars::JsonSchema;
use serde::Deserialize;
use std::fs::File;
use std::io::Read;

use crate::core::error::{AgentError, Result};
use crate::tools::ToolType;
use crate::tools::TypedTool;

use super::constants::{
    READ_BINARY_CHECK_SIZE, READ_DEFAULT_LIMIT, READ_DEFAULT_OFFSET, READ_MAX_LIMIT,
    READ_MAX_LINE_LENGTH,
};
use super::{validate_absolute_path, validate_file_size, validate_is_file, validate_path_exists};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadFileInput {
    pub path: String,

    #[serde(default = "read_default_offset")]
    pub offset: usize,

    #[serde(default = "read_default_limit")]
    pub limit: usize,
}

const fn read_default_offset() -> usize {
    READ_DEFAULT_OFFSET
}

const fn read_default_limit() -> usize {
    READ_DEFAULT_LIMIT
}

pub struct ReadFileTool;

impl ReadFileTool {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for ReadFileTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TypedTool for ReadFileTool {
    type Input = ReadFileInput;

    fn name(&self) -> &'static str {
        "read_file"
    }

    fn description(&self) -> &'static str {
        "Read the contents of a file with optional offset and limit. Returns line-numbered content. The path must be absolute."
    }

    async fn execute_typed(&self, input: Self::Input) -> Result<String> {
        let path = validate_absolute_path(&input.path, ToolType::ReadFile)?;

        let limit = input.limit.min(READ_MAX_LIMIT);

        validate_path_exists(&path, ToolType::ReadFile)?;
        validate_is_file(&path, ToolType::ReadFile)?;
        let file_size = validate_file_size(&path, ToolType::ReadFile)?;

        let mut file = File::open(&path)?;
        let mut buffer = vec![0u8; READ_BINARY_CHECK_SIZE.min(file_size as usize)];
        let bytes_read = file.read(&mut buffer)?;
        buffer.truncate(bytes_read);

        if content_inspector::inspect(&buffer) == ContentType::BINARY {
            return Err(AgentError::InvalidToolInput {
                tool: ToolType::ReadFile.name().to_string(),
                reason: format!(
                    "File appears to be binary: {}. Use a hex viewer or appropriate tool instead.",
                    path.display()
                ),
            });
        }

        let contents = std::fs::read_to_string(&path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::InvalidData {
                AgentError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("File is not valid UTF-8: {}", path.display()),
                ))
            } else {
                AgentError::Io(e)
            }
        })?;

        let all_lines: Vec<&str> = contents.lines().collect();
        let total_lines = all_lines.len();

        let start_idx = if input.offset > 0 {
            input.offset.saturating_sub(1)
        } else {
            0
        };

        if start_idx >= total_lines {
            return Ok(format!(
                "File: {}\n\nOffset {} is beyond end of file ({} lines total)",
                path.display(),
                input.offset,
                total_lines
            ));
        }

        let end_idx = std::cmp::min(start_idx + limit, total_lines);
        let selected_lines = &all_lines[start_idx..end_idx];

        let mut output = String::new();
        output.push_str(&format!("File: {}\n", path.display()));
        output.push_str(&format!(
            "Lines {}-{} of {} total\n\n",
            input.offset,
            start_idx + selected_lines.len(),
            total_lines
        ));

        for (idx, line) in selected_lines.iter().enumerate() {
            let line_num = start_idx + idx + 1;

            let formatted_line = if line.len() > READ_MAX_LINE_LENGTH {
                format!("{}... [truncated]", &line[..READ_MAX_LINE_LENGTH])
            } else {
                (*line).to_string()
            };

            output.push_str(&format!("L{line_num}: {formatted_line}\n"));
        }

        if selected_lines.len() < total_lines {
            output.push_str(&format!(
                "\n[Showing lines {}-{} of {} total]",
                input.offset,
                start_idx + selected_lines.len(),
                total_lines
            ));
        }

        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::Tool;
    use std::io::Write;

    #[test]
    fn test_read_file_tool_name() {
        let tool = ReadFileTool::new();
        assert_eq!(Tool::name(&tool), "read_file");
    }

    #[tokio::test]
    async fn test_read_file_basic() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        let mut file = std::fs::File::create(&file_path).unwrap();
        writeln!(file, "Line 1").unwrap();
        writeln!(file, "Line 2").unwrap();
        writeln!(file, "Line 3").unwrap();

        let tool = ReadFileTool::new();
        let input = ReadFileInput {
            path: file_path.to_str().unwrap().to_string(),
            offset: 1,
            limit: 100,
        };

        let result = tool.execute_typed(input).await.unwrap();
        assert!(result.contains("L1: Line 1"));
        assert!(result.contains("L2: Line 2"));
        assert!(result.contains("L3: Line 3"));
    }

    #[tokio::test]
    async fn test_read_file_with_offset() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        let mut file = std::fs::File::create(&file_path).unwrap();
        writeln!(file, "Line 1").unwrap();
        writeln!(file, "Line 2").unwrap();
        writeln!(file, "Line 3").unwrap();

        let tool = ReadFileTool::new();
        let input = ReadFileInput {
            path: file_path.to_str().unwrap().to_string(),
            offset: 2,
            limit: 100,
        };

        let result = tool.execute_typed(input).await.unwrap();
        assert!(!result.contains("L1: Line 1"));
        assert!(result.contains("L2: Line 2"));
        assert!(result.contains("L3: Line 3"));
    }

    #[tokio::test]
    async fn test_read_file_with_limit() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        let mut file = std::fs::File::create(&file_path).unwrap();
        for i in 1..=10 {
            writeln!(file, "Line {i}").unwrap();
        }

        let tool = ReadFileTool::new();
        let input = ReadFileInput {
            path: file_path.to_str().unwrap().to_string(),
            offset: 1,
            limit: 3,
        };

        let result = tool.execute_typed(input).await.unwrap();
        assert!(result.contains("L1: Line 1"));
        assert!(result.contains("L2: Line 2"));
        assert!(result.contains("L3: Line 3"));
        assert!(!result.contains("L4: Line 4"));
        assert!(result.contains("Showing lines 1-3 of 10 total"));
    }

    #[tokio::test]
    async fn test_read_file_line_truncation() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        let mut file = std::fs::File::create(&file_path).unwrap();
        let long_line = "x".repeat(1000);
        writeln!(file, "{long_line}").unwrap();

        let tool = ReadFileTool::new();
        let input = ReadFileInput {
            path: file_path.to_str().unwrap().to_string(),
            offset: 1,
            limit: 100,
        };

        let result = tool.execute_typed(input).await.unwrap();
        assert!(result.contains("truncated"));
        assert!(!result.contains(&"x".repeat(1000)));
    }

    #[tokio::test]
    async fn test_read_file_not_found() {
        let tool = ReadFileTool::new();
        let input = ReadFileInput {
            path: "/nonexistent/file.txt".to_string(),
            offset: 1,
            limit: 100,
        };

        let result = tool.execute_typed(input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_read_binary_file_rejected() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("binary.bin");

        std::fs::write(&file_path, &[0u8, 1, 2, 3, 0, 0, 255, 254]).unwrap();

        let tool = ReadFileTool::new();
        let input = ReadFileInput {
            path: file_path.to_str().unwrap().to_string(),
            offset: 1,
            limit: 100,
        };

        let result = tool.execute_typed(input).await;
        assert!(result.is_err());

        if let Err(AgentError::InvalidToolInput { reason, .. }) = result {
            assert!(reason.contains("binary"));
        } else {
            panic!("Expected InvalidToolInput error for binary file");
        }
    }
}
