use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use std::path::{Path, PathBuf};

use crate::core::error::{AgentError, Result};
use crate::core::metadata;
use crate::tools::{ToolType, TypedTool};

use super::{atomic_write, validate_absolute_path, validate_file_size, validate_path_exists};
#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateFileInput {
    pub path: String,
    pub old_string: String,
    pub new_string: String,
    #[serde(default)]
    pub replace_all: bool,
}
enum UpdateResult {
    NoChange,
    Updated {
        path: PathBuf,
        old_content: String,
        new_content: String,
        occurrences: usize,
    },
}

enum MatchValidation {
    NotFound,
    Ambiguous(usize),
    Valid(usize),
}
fn validate_matches(content: &str, needle: &str, replace_all: bool) -> MatchValidation {
    let count = content.matches(needle).count();

    match count {
        0 => MatchValidation::NotFound,
        1 => MatchValidation::Valid(1),
        n if replace_all => MatchValidation::Valid(n),
        n => MatchValidation::Ambiguous(n),
    }
}

fn perform_replacement(content: &str, old: &str, new: &str, replace_all: bool) -> String {
    if replace_all {
        content.replace(old, new)
    } else {
        content.replacen(old, new, 1)
    }
}

fn execute_update(
    path: &Path,
    old_string: &str,
    new_string: &str,
    replace_all: bool,
) -> Result<UpdateResult> {
    let old_content = std::fs::read_to_string(path)?;

    let occurrences = match validate_matches(&old_content, old_string, replace_all) {
        MatchValidation::NotFound => {
            return Err(AgentError::InvalidToolInput {
                tool: ToolType::UpdateFile.name().to_string(),
                reason: format!(
                    "Could not find old_string in file. Make sure it matches exactly:\n{old_string}"
                ),
            });
        }
        MatchValidation::Ambiguous(count) => {
            return Err(AgentError::InvalidToolInput {
                tool: ToolType::UpdateFile.name().to_string(),
                reason: format!(
                    "Found {count} occurrences of old_string. \
                     Use replace_all=true to replace all, or provide a longer, unique string."
                ),
            });
        }
        MatchValidation::Valid(count) => count,
    };

    let new_content = perform_replacement(&old_content, old_string, new_string, replace_all);

    if old_content == new_content {
        return Ok(UpdateResult::NoChange);
    }

    atomic_write(path, &new_content)?;

    Ok(UpdateResult::Updated {
        path: path.to_path_buf(),
        old_content,
        new_content,
        occurrences,
    })
}
fn format_output(result: UpdateResult, path: &Path) -> String {
    match result {
        UpdateResult::NoChange => {
            format!(
                "No changes made to {} (old_string and new_string are identical)",
                path.display()
            )
        }
        UpdateResult::Updated {
            path,
            old_content,
            new_content,
            occurrences,
        } => {
            let plural = if occurrences == 1 { "" } else { "s" };
            let summary = format!(
                "Updated {} ({occurrences} occurrence{plural})",
                path.display()
            );

            let metadata_json = json!({
                "diff_metadata": {
                    "path": path.to_string_lossy(),
                    "old_content": old_content,
                    "new_content": new_content,
                }
            });

            format!(
                "{summary}\n\n{}",
                metadata::wrap(&metadata_json.to_string())
            )
        }
    }
}
#[derive(Default)]
pub struct UpdateFileTool;

impl UpdateFileTool {
    #[must_use]
    pub const fn new() -> Self {
        Self
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
        let path = validate_absolute_path(&input.path, &ToolType::UpdateFile)?;
        validate_path_exists(&path, &ToolType::UpdateFile)?;
        validate_file_size(&path, &ToolType::UpdateFile)?;

        let result = execute_update(
            &path,
            &input.old_string,
            &input.new_string,
            input.replace_all,
        )?;

        Ok(format_output(result, &path))
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
