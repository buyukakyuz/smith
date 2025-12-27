use async_trait::async_trait;
use globset::{Glob, GlobMatcher};
use schemars::JsonSchema;
use serde::Deserialize;
use std::fmt::{self, Display};
use std::path::{Path, PathBuf};
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
struct FileMatch {
    path: PathBuf,
    modified: SystemTime,
}

impl FileMatch {
    fn relative_path<'a>(&'a self, base: &Path) -> &'a Path {
        self.path.strip_prefix(base).unwrap_or(&self.path)
    }
}

fn matches_glob(path: &Path, base_dir: &Path, matcher: &GlobMatcher) -> bool {
    let relative = path.strip_prefix(base_dir).unwrap_or(path);
    matcher.is_match(relative) || path.file_name().is_some_and(|name| matcher.is_match(name))
}

fn collect_matches(
    base_dir: &Path,
    matcher: &GlobMatcher,
    respect_gitignore: bool,
) -> Vec<FileMatch> {
    walk_builder_with_gitignore(base_dir, respect_gitignore)
        .build()
        .flatten()
        .filter(|entry| !entry.path().is_dir())
        .filter(|entry| matches_glob(entry.path(), base_dir, matcher))
        .filter_map(|entry| {
            entry.metadata().ok().map(|meta| FileMatch {
                path: entry.path().to_path_buf(),
                modified: meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            })
        })
        .collect()
}
struct GlobOutput<'a> {
    pattern: &'a str,
    results: &'a [FileMatch],
    base_dir: &'a Path,
    total_found: usize,
    respect_gitignore: bool,
}

impl Display for GlobOutput<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.results.is_empty() {
            return write!(f, "No files found matching pattern: {}", self.pattern);
        }

        writeln!(
            f,
            "Found {} files matching \"{}\":\n",
            self.total_found, self.pattern
        )?;

        for file in self.results {
            let relative = file.relative_path(self.base_dir).display();
            let modified = format_time_ago(file.modified);
            writeln!(f, "{relative} (modified {modified})")?;
        }

        write!(
            f,
            "\n[Showing {} of {} results]",
            self.results.len(),
            self.total_found
        )?;
        write!(f, "\n[Pattern: {}]", self.pattern)?;

        if self.respect_gitignore {
            write!(f, "\n[Respecting .gitignore]")?;
        }

        Ok(())
    }
}
#[derive(Default)]
pub struct GlobTool;

impl GlobTool {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    fn build_matcher(pattern: &str) -> Result<GlobMatcher> {
        Glob::new(pattern)
            .map(|g| g.compile_matcher())
            .map_err(|e| AgentError::InvalidToolInput {
                tool: ToolType::Glob.name().to_string(),
                reason: format!("Invalid glob pattern: {e}"),
            })
    }

    fn resolve_base_dir(base_dir: Option<&str>) -> Result<PathBuf> {
        match base_dir {
            Some(base) => {
                let path = validate_absolute_path(base, &ToolType::Glob)?;
                validate_path_exists(&path, &ToolType::Glob)?;
                Ok(path)
            }
            None => Ok(std::env::current_dir()?),
        }
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
        let matcher = Self::build_matcher(&input.pattern)?;
        let base_dir = Self::resolve_base_dir(input.base_dir.as_deref())?;

        let mut results = collect_matches(&base_dir, &matcher, input.respect_gitignore);
        let total_found = results.len();

        results.sort_unstable_by(|a, b| b.modified.cmp(&a.modified));
        results.truncate(limit);

        let output = GlobOutput {
            pattern: &input.pattern,
            results: &results,
            base_dir: &base_dir,
            total_found,
            respect_gitignore: input.respect_gitignore,
        };

        Ok(output.to_string())
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
