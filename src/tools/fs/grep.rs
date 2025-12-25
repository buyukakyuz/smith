use async_trait::async_trait;
use globset::Glob;
use grep_regex::RegexMatcherBuilder;
use grep_searcher::sinks::UTF8;
use grep_searcher::{BinaryDetection, Searcher, SearcherBuilder};
use schemars::JsonSchema;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::core::error::{AgentError, Result};
use crate::tools::{ToolType, TypedTool};

use super::constants::{
    GREP_DEFAULT_LIMIT, GREP_MAX_CONTEXT, GREP_MAX_LIMIT, default_respect_gitignore,
};
use super::{validate_absolute_path, validate_path_exists, walk_builder_with_gitignore};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GrepInput {
    pub pattern: String,

    #[serde(default)]
    pub path: Option<String>,

    #[serde(default)]
    pub glob: Option<String>,

    #[serde(default)]
    pub ignore_case: bool,

    #[serde(default = "grep_default_limit")]
    pub limit: usize,

    #[serde(default)]
    pub context: usize,

    #[serde(default = "default_respect_gitignore")]
    #[schemars(default = "default_respect_gitignore")]
    pub respect_gitignore: bool,
}

const fn grep_default_limit() -> usize {
    GREP_DEFAULT_LIMIT
}

#[derive(Clone)]
struct MatchResult {
    path: PathBuf,
    line_number: u64,
    line: String,
    context_before: Vec<String>,
    context_after: Vec<String>,
}

pub struct GrepTool;

impl GrepTool {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    fn search_file(
        path: &Path,
        matcher: &grep_regex::RegexMatcher,
        searcher: &mut Searcher,
        context: usize,
        limit: usize,
        results: &Arc<Mutex<Vec<MatchResult>>>,
    ) -> bool {
        let file_contents = std::fs::read_to_string(path).unwrap_or_default();
        let lines: Vec<&str> = file_contents.lines().collect();

        let search_result = searcher.search_path(
            matcher,
            path,
            UTF8(|line_num, line| {
                let mut results_guard = results.lock().unwrap();

                if results_guard.len() >= limit {
                    return Ok(false);
                }

                let line_idx = (line_num as usize).saturating_sub(1);

                let context_before: Vec<String> = if context > 0 && line_idx > 0 {
                    let start = line_idx.saturating_sub(context);
                    lines[start..line_idx]
                        .iter()
                        .map(|s| (*s).to_string())
                        .collect()
                } else {
                    Vec::new()
                };

                let context_after: Vec<String> = if context > 0 {
                    let end = std::cmp::min(line_idx + 1 + context, lines.len());
                    if line_idx + 1 < lines.len() {
                        lines[line_idx + 1..end]
                            .iter()
                            .map(|s| (*s).to_string())
                            .collect()
                    } else {
                        Vec::new()
                    }
                } else {
                    Vec::new()
                };

                results_guard.push(MatchResult {
                    path: path.to_path_buf(),
                    line_number: line_num,
                    line: line.trim_end().to_string(),
                    context_before,
                    context_after,
                });

                Ok(true)
            }),
        );

        if search_result.is_err() {
            return true;
        }

        let results_guard = results.lock().unwrap();
        results_guard.len() < limit
    }
}

impl Default for GrepTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TypedTool for GrepTool {
    type Input = GrepInput;

    fn name(&self) -> &'static str {
        "grep"
    }

    fn description(&self) -> &'static str {
        "Search file contents using regex patterns. Supports recursive directory search, glob filtering, and context lines. Respects .gitignore by default. Returns file:line:content format."
    }

    async fn execute_typed(&self, input: Self::Input) -> Result<String> {
        let limit = input.limit.min(GREP_MAX_LIMIT);
        let context = input.context.min(GREP_MAX_CONTEXT);

        let matcher = RegexMatcherBuilder::new()
            .case_insensitive(input.ignore_case)
            .build(&input.pattern)
            .map_err(|e| AgentError::InvalidToolInput {
                tool: ToolType::Grep.name().to_string(),
                reason: format!("Invalid regex pattern: {e}"),
            })?;

        let mut searcher = SearcherBuilder::new()
            .binary_detection(BinaryDetection::quit(0x00))
            .line_number(true)
            .build();

        let search_path = if let Some(path_str) = &input.path {
            let path = validate_absolute_path(path_str, ToolType::Grep)?;
            validate_path_exists(&path, ToolType::Grep)?;
            path
        } else {
            std::env::current_dir()?
        };

        let glob_matcher = if let Some(glob_pattern) = &input.glob {
            Some(
                Glob::new(glob_pattern)
                    .map_err(|e| AgentError::InvalidToolInput {
                        tool: ToolType::Grep.name().to_string(),
                        reason: format!("Invalid glob pattern: {e}"),
                    })?
                    .compile_matcher(),
            )
        } else {
            None
        };

        let results: Arc<Mutex<Vec<MatchResult>>> = Arc::new(Mutex::new(Vec::new()));

        if search_path.is_file() {
            Self::search_file(
                &search_path,
                &matcher,
                &mut searcher,
                context,
                limit,
                &results,
            );
        } else {
            let walker = walk_builder_with_gitignore(&search_path, input.respect_gitignore).build();

            for entry in walker.flatten() {
                let entry_path = entry.path();

                if entry_path.is_dir() {
                    continue;
                }

                if let Some(ref glob) = glob_matcher
                    && let Some(file_name) = entry_path.file_name()
                    && !glob.is_match(file_name)
                {
                    continue;
                }

                let should_continue = Self::search_file(
                    entry_path,
                    &matcher,
                    &mut searcher,
                    context,
                    limit,
                    &results,
                );

                if !should_continue {
                    break;
                }
            }
        }

        let results = results.lock().unwrap();

        if results.is_empty() {
            return Ok(format!(
                "No matches found for pattern: \"{}\"",
                input.pattern
            ));
        }

        let mut output = String::new();
        output.push_str(&format!(
            "Found {} matches for \"{}\":\n\n",
            results.len(),
            input.pattern
        ));

        for result in results.iter() {
            output.push_str(&format!(
                "{}:{}:",
                result.path.display(),
                result.line_number
            ));

            if context > 0 {
                output.push('\n');
                for line in &result.context_before {
                    output.push_str(&format!("   {line}\n"));
                }
                output.push_str(&format!(" > {}\n", result.line));
                for line in &result.context_after {
                    output.push_str(&format!("   {line}\n"));
                }
            } else {
                output.push_str(&format!(" {}\n", result.line));
            }
        }

        output.push_str(&format!(
            "\n[Showing {} of {} matches]",
            results.len(),
            results.len()
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
    use std::fs;
    use std::io::Write;

    #[test]
    fn test_grep_tool_name() {
        let tool = GrepTool::new();
        assert_eq!(Tool::name(&tool), "grep");
    }

    #[tokio::test]
    async fn test_grep_basic() {
        let temp_dir = tempfile::tempdir().unwrap();
        let dir_path = temp_dir.path();
        let file_path = dir_path.join("test.txt");

        let mut file = fs::File::create(&file_path).unwrap();
        writeln!(file, "hello world").unwrap();
        writeln!(file, "foo bar").unwrap();
        writeln!(file, "hello again").unwrap();

        let tool = GrepTool::new();
        let input = GrepInput {
            pattern: "hello".to_string(),
            path: Some(file_path.to_str().unwrap().to_string()),
            glob: None,
            ignore_case: false,
            limit: 100,
            context: 0,
            respect_gitignore: false,
        };

        let result = tool.execute_typed(input).await.unwrap();
        assert!(result.contains("hello world"));
        assert!(result.contains("hello again"));
        assert!(!result.contains("foo bar"));
    }

    #[tokio::test]
    async fn test_grep_case_insensitive() {
        let temp_dir = tempfile::tempdir().unwrap();
        let dir_path = temp_dir.path();
        let file_path = dir_path.join("test.txt");

        let mut file = fs::File::create(&file_path).unwrap();
        writeln!(file, "HELLO world").unwrap();
        writeln!(file, "foo bar").unwrap();

        let tool = GrepTool::new();
        let input = GrepInput {
            pattern: "hello".to_string(),
            path: Some(file_path.to_str().unwrap().to_string()),
            glob: None,
            ignore_case: true,
            limit: 100,
            context: 0,
            respect_gitignore: false,
        };

        let result = tool.execute_typed(input).await.unwrap();
        assert!(result.contains("HELLO world"));
    }

    #[tokio::test]
    async fn test_grep_with_context() {
        let temp_dir = tempfile::tempdir().unwrap();
        let dir_path = temp_dir.path();
        let file_path = dir_path.join("test.txt");

        let mut file = fs::File::create(&file_path).unwrap();
        writeln!(file, "line 1").unwrap();
        writeln!(file, "line 2 match").unwrap();
        writeln!(file, "line 3").unwrap();

        let tool = GrepTool::new();
        let input = GrepInput {
            pattern: "match".to_string(),
            path: Some(file_path.to_str().unwrap().to_string()),
            glob: None,
            ignore_case: false,
            limit: 100,
            context: 1,
            respect_gitignore: false,
        };

        let result = tool.execute_typed(input).await.unwrap();
        assert!(result.contains("line 1"));
        assert!(result.contains("line 2 match"));
        assert!(result.contains("line 3"));
    }

    #[tokio::test]
    async fn test_grep_directory() {
        let temp_dir = tempfile::tempdir().unwrap();
        let dir_path = temp_dir.path();

        let mut file1 = fs::File::create(dir_path.join("file1.txt")).unwrap();
        writeln!(file1, "hello world").unwrap();

        let mut file2 = fs::File::create(dir_path.join("file2.txt")).unwrap();
        writeln!(file2, "foo bar").unwrap();
        writeln!(file2, "hello again").unwrap();

        let tool = GrepTool::new();
        let input = GrepInput {
            pattern: "hello".to_string(),
            path: Some(dir_path.to_str().unwrap().to_string()),
            glob: None,
            ignore_case: false,
            limit: 100,
            context: 0,
            respect_gitignore: false,
        };

        let result = tool.execute_typed(input).await.unwrap();
        assert!(result.contains("file1.txt"));
        assert!(result.contains("file2.txt"));
        assert!(result.contains("hello world"));
        assert!(result.contains("hello again"));
    }

    #[tokio::test]
    async fn test_grep_no_matches() {
        let temp_dir = tempfile::tempdir().unwrap();
        let dir_path = temp_dir.path();
        let file_path = dir_path.join("test.txt");

        let mut file = fs::File::create(&file_path).unwrap();
        writeln!(file, "hello world").unwrap();

        let tool = GrepTool::new();
        let input = GrepInput {
            pattern: "nonexistent".to_string(),
            path: Some(file_path.to_str().unwrap().to_string()),
            glob: None,
            ignore_case: false,
            limit: 100,
            context: 0,
            respect_gitignore: false,
        };

        let result = tool.execute_typed(input).await.unwrap();
        assert!(result.contains("No matches found"));
    }

    #[tokio::test]
    async fn test_grep_respects_gitignore() {
        let temp_dir = tempfile::tempdir().unwrap();
        let dir_path = temp_dir.path();

        std::fs::create_dir(dir_path.join(".git")).unwrap();

        std::fs::write(dir_path.join(".gitignore"), "ignored.txt\n").unwrap();

        let mut visible = fs::File::create(dir_path.join("visible.txt")).unwrap();
        writeln!(visible, "hello from visible").unwrap();

        let mut ignored = fs::File::create(dir_path.join("ignored.txt")).unwrap();
        writeln!(ignored, "hello from ignored").unwrap();

        let tool = GrepTool::new();

        let input = GrepInput {
            pattern: "hello".to_string(),
            path: Some(dir_path.to_str().unwrap().to_string()),
            glob: None,
            ignore_case: false,
            limit: 100,
            context: 0,
            respect_gitignore: true,
        };
        let result = tool.execute_typed(input).await.unwrap();
        assert!(result.contains("visible.txt"));
        assert!(!result.contains("ignored.txt"));

        let input = GrepInput {
            pattern: "hello".to_string(),
            path: Some(dir_path.to_str().unwrap().to_string()),
            glob: None,
            ignore_case: false,
            limit: 100,
            context: 0,
            respect_gitignore: false,
        };
        let result = tool.execute_typed(input).await.unwrap();
        assert!(result.contains("visible.txt"));
        assert!(result.contains("ignored.txt"));
    }
}
