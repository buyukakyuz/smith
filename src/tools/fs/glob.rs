use async_trait::async_trait;
use globset::{Glob, GlobSetBuilder};
use schemars::JsonSchema;
use serde::Deserialize;
use std::path::PathBuf;
use std::time::SystemTime;

use crate::core::error::{AgentError, Result};
use crate::tools::{ToolType, TypedTool};

use super::constants::{GLOB_DEFAULT_LIMIT, GLOB_MAX_LIMIT, default_respect_gitignore};
use super::format::format_time_ago;
use super::{validate_absolute_path, validate_path_exists, walk_builder_with_gitignore};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GlobInput {
    pub pattern: String,

    #[serde(default)]
    pub base_dir: Option<String>,

    #[serde(default = "glob_default_limit")]
    pub limit: usize,

    #[serde(default = "default_respect_gitignore")]
    #[schemars(default = "default_respect_gitignore")]
    pub respect_gitignore: bool,
}

const fn glob_default_limit() -> usize {
    GLOB_DEFAULT_LIMIT
}

pub struct GlobTool;

impl GlobTool {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for GlobTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TypedTool for GlobTool {
    type Input = GlobInput;

    fn name(&self) -> &'static str {
        "glob"
    }

    fn description(&self) -> &'static str {
        "Find files matching glob patterns. Supports wildcards (* and **) and brace expansion. Returns paths sorted by modification time (newest first). Respects .gitignore by default."
    }

    async fn execute_typed(&self, input: Self::Input) -> Result<String> {
        let limit = input.limit.min(GLOB_MAX_LIMIT);

        let base_dir = if let Some(base) = &input.base_dir {
            let base_path = validate_absolute_path(base, &ToolType::Glob)?;
            validate_path_exists(&base_path, &ToolType::Glob)?;
            base_path
        } else {
            std::env::current_dir()?
        };

        let glob = Glob::new(&input.pattern).map_err(|e| AgentError::InvalidToolInput {
            tool: ToolType::Glob.name().to_string(),
            reason: format!("Invalid glob pattern: {e}"),
        })?;

        let mut builder = GlobSetBuilder::new();
        builder.add(glob);
        let glob_set = builder.build().map_err(|e| AgentError::InvalidToolInput {
            tool: ToolType::Glob.name().to_string(),
            reason: format!("Failed to build glob set: {e}"),
        })?;

        let walker = walk_builder_with_gitignore(&base_dir, input.respect_gitignore).build();

        let mut results: Vec<(PathBuf, SystemTime)> = Vec::new();

        for entry in walker.flatten() {
            let entry_path = entry.path();

            if entry_path.is_dir() {
                continue;
            }

            let rel_path = entry_path.strip_prefix(&base_dir).unwrap_or(entry_path);

            let matches = glob_set.is_match(rel_path)
                || entry_path
                    .file_name()
                    .is_some_and(|name| glob_set.is_match(name));

            if !matches {
                continue;
            }

            if let Ok(metadata) = entry.metadata() {
                let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                results.push((entry_path.to_path_buf(), modified));
            }

            if results.len() >= limit {
                break;
            }
        }

        results.sort_by(|a, b| b.1.cmp(&a.1));

        if results.is_empty() {
            return Ok(format!(
                "No files found matching pattern: {}",
                input.pattern
            ));
        }

        let mut output = String::new();
        output.push_str(&format!(
            "Found {} files matching \"{}\":\n\n",
            results.len(),
            input.pattern
        ));

        for (path, modified) in &results {
            let relative_path = path
                .strip_prefix(&base_dir)
                .unwrap_or(path)
                .to_string_lossy()
                .to_string();

            output.push_str(&format!(
                "{} (modified {})\n",
                relative_path,
                format_time_ago(*modified)
            ));
        }

        let total_matches = results.len();
        output.push_str(&format!(
            "\n[Showing {} of {} results]",
            total_matches.min(limit),
            total_matches
        ));
        output.push_str(&format!("\n[Pattern: {}]", input.pattern));
        if input.respect_gitignore {
            output.push_str("\n[Respecting .gitignore]");
        }

        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::Tool;

    #[test]
    fn test_glob_tool_name() {
        let tool = GlobTool::new();
        assert_eq!(Tool::name(&tool), "glob");
    }

    #[tokio::test]
    async fn test_glob_basic() {
        let temp_dir = tempfile::tempdir().unwrap();
        let dir_path = temp_dir.path();

        std::fs::File::create(dir_path.join("test1.txt")).unwrap();
        std::fs::File::create(dir_path.join("test2.txt")).unwrap();
        std::fs::File::create(dir_path.join("other.rs")).unwrap();

        let tool = GlobTool::new();
        let input = GlobInput {
            pattern: "*.txt".to_string(),
            base_dir: Some(dir_path.to_str().unwrap().to_string()),
            limit: 100,
            respect_gitignore: false,
        };

        let result = tool.execute_typed(input).await.unwrap();
        assert!(result.contains("test1.txt"));
        assert!(result.contains("test2.txt"));
        assert!(!result.contains("other.rs"));
    }

    #[tokio::test]
    async fn test_glob_recursive() {
        let temp_dir = tempfile::tempdir().unwrap();
        let dir_path = temp_dir.path();

        std::fs::create_dir(dir_path.join("subdir")).unwrap();
        std::fs::File::create(dir_path.join("test.rs")).unwrap();
        std::fs::File::create(dir_path.join("subdir").join("nested.rs")).unwrap();

        let tool = GlobTool::new();
        let input = GlobInput {
            pattern: "**/*.rs".to_string(),
            base_dir: Some(dir_path.to_str().unwrap().to_string()),
            limit: 100,
            respect_gitignore: false,
        };

        let result = tool.execute_typed(input).await.unwrap();
        assert!(result.contains("test.rs"));
        assert!(result.contains("nested.rs"));
    }

    #[tokio::test]
    async fn test_glob_with_base_dir() {
        let temp_dir = tempfile::tempdir().unwrap();
        let dir_path = temp_dir.path();

        std::fs::File::create(dir_path.join("test1.txt")).unwrap();
        std::fs::File::create(dir_path.join("test2.txt")).unwrap();

        let tool = GlobTool::new();
        let input = GlobInput {
            pattern: "*.txt".to_string(),
            base_dir: Some(dir_path.to_str().unwrap().to_string()),
            limit: 100,
            respect_gitignore: false,
        };

        let result = tool.execute_typed(input).await.unwrap();
        assert!(result.contains("test1.txt"));
        assert!(result.contains("test2.txt"));
    }

    #[tokio::test]
    async fn test_glob_limit() {
        let temp_dir = tempfile::tempdir().unwrap();
        let dir_path = temp_dir.path();

        for i in 1..=10 {
            std::fs::File::create(dir_path.join(format!("test{i}.txt"))).unwrap();
        }

        let tool = GlobTool::new();
        let input = GlobInput {
            pattern: "*.txt".to_string(),
            base_dir: Some(dir_path.to_str().unwrap().to_string()),
            limit: 5,
            respect_gitignore: false,
        };

        let result = tool.execute_typed(input).await.unwrap();
        assert!(result.contains("Showing 5 of 5 results"));
    }

    #[tokio::test]
    async fn test_glob_no_matches() {
        let temp_dir = tempfile::tempdir().unwrap();
        let dir_path = temp_dir.path();

        let tool = GlobTool::new();
        let input = GlobInput {
            pattern: "*.nonexistent".to_string(),
            base_dir: Some(dir_path.to_str().unwrap().to_string()),
            limit: 100,
            respect_gitignore: false,
        };

        let result = tool.execute_typed(input).await.unwrap();
        assert!(result.contains("No files found"));
    }

    #[tokio::test]
    async fn test_glob_respects_gitignore() {
        let temp_dir = tempfile::tempdir().unwrap();
        let dir_path = temp_dir.path();

        std::fs::create_dir(dir_path.join(".git")).unwrap();

        std::fs::write(dir_path.join(".gitignore"), "ignored.txt\n").unwrap();

        std::fs::File::create(dir_path.join("visible.txt")).unwrap();
        std::fs::File::create(dir_path.join("ignored.txt")).unwrap();

        let tool = GlobTool::new();

        let input = GlobInput {
            pattern: "*.txt".to_string(),
            base_dir: Some(dir_path.to_str().unwrap().to_string()),
            limit: 100,
            respect_gitignore: true,
        };
        let result = tool.execute_typed(input).await.unwrap();
        assert!(result.contains("visible.txt"));
        assert!(!result.contains("ignored.txt"));

        let input = GlobInput {
            pattern: "*.txt".to_string(),
            base_dir: Some(dir_path.to_str().unwrap().to_string()),
            limit: 100,
            respect_gitignore: false,
        };
        let result = tool.execute_typed(input).await.unwrap();
        assert!(result.contains("visible.txt"));
        assert!(result.contains("ignored.txt"));
    }
}
