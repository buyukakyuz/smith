use async_trait::async_trait;
use globset::{Glob, GlobMatcher};
use grep_regex::RegexMatcherBuilder;
use grep_searcher::sinks::UTF8;
use grep_searcher::{BinaryDetection, Searcher, SearcherBuilder};
use schemars::JsonSchema;
use serde::Deserialize;
use std::fmt::{self};
use std::path::{Path, PathBuf};

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
struct MatchResult {
    path: PathBuf,
    line_number: u64,
    line: String,
    context_before: Vec<String>,
    context_after: Vec<String>,
}

struct SearchConfig {
    context: usize,
    limit: usize,
}

struct MatchCollector {
    matches: Vec<MatchResult>,
    remaining: usize,
}

impl MatchCollector {
    fn with_capacity(limit: usize) -> Self {
        Self {
            matches: Vec::with_capacity(limit.min(64)),
            remaining: limit,
        }
    }

    const fn is_full(&self) -> bool {
        self.remaining == 0
    }

    fn push(&mut self, result: MatchResult) -> bool {
        if self.remaining == 0 {
            return false;
        }
        self.matches.push(result);
        self.remaining -= 1;
        self.remaining > 0
    }

    fn into_vec(self) -> Vec<MatchResult> {
        self.matches
    }
}
fn search_file(
    path: &Path,
    matcher: &grep_regex::RegexMatcher,
    searcher: &mut Searcher,
    config: &SearchConfig,
    collector: &mut MatchCollector,
) {
    let lines: Option<Vec<String>> = if config.context > 0 {
        std::fs::read_to_string(path)
            .ok()
            .map(|contents| contents.lines().map(String::from).collect())
    } else {
        None
    };

    let _ = searcher.search_path(
        matcher,
        path,
        UTF8(|line_num, line| {
            if collector.is_full() {
                return Ok(false);
            }

            let (context_before, context_after) = lines
                .as_ref()
                .map(|lines| extract_context(lines, line_num as usize, config.context))
                .unwrap_or_default();

            let should_continue = collector.push(MatchResult {
                path: path.to_path_buf(),
                line_number: line_num,
                line: line.trim_end().to_string(),
                context_before,
                context_after,
            });

            Ok(should_continue)
        }),
    );
}

fn extract_context(
    lines: &[String],
    line_num: usize,
    context: usize,
) -> (Vec<String>, Vec<String>) {
    let idx = line_num.saturating_sub(1);

    let before = (idx.saturating_sub(context)..idx)
        .filter_map(|i| lines.get(i).cloned())
        .collect();

    let after = (idx + 1..=idx + context)
        .filter_map(|i| lines.get(i).cloned())
        .collect();

    (before, after)
}

fn matches_glob(path: &Path, glob: Option<&GlobMatcher>) -> bool {
    glob.is_none_or(|g| path.file_name().is_some_and(|name| g.is_match(name)))
}

struct SearchOutput<'a> {
    pattern: &'a str,
    results: &'a [MatchResult],
    context: usize,
    respect_gitignore: bool,
}

impl fmt::Display for SearchOutput<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.results.is_empty() {
            return write!(f, "No matches found for pattern: \"{}\"", self.pattern);
        }

        writeln!(
            f,
            "Found {} matches for \"{}\":\n",
            self.results.len(),
            self.pattern
        )?;

        for result in self.results {
            write!(f, "{}:{}:", result.path.display(), result.line_number)?;

            if self.context > 0 {
                writeln!(f)?;
                for line in &result.context_before {
                    writeln!(f, "   {line}")?;
                }
                writeln!(f, " > {}", result.line)?;
                for line in &result.context_after {
                    writeln!(f, "   {line}")?;
                }
            } else {
                writeln!(f, " {}", result.line)?;
            }
        }

        write!(
            f,
            "\n[Showing {} of {} matches]",
            self.results.len(),
            self.results.len()
        )?;
        write!(f, "\n[Pattern: {}]", self.pattern)?;

        if self.respect_gitignore {
            write!(f, "\n[Respecting .gitignore]")?;
        }

        Ok(())
    }
}

#[derive(Default)]
pub struct GrepTool;

impl GrepTool {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    fn build_matcher(pattern: &str, ignore_case: bool) -> Result<grep_regex::RegexMatcher> {
        RegexMatcherBuilder::new()
            .case_insensitive(ignore_case)
            .build(pattern)
            .map_err(|e| AgentError::InvalidToolInput {
                tool: ToolType::Grep.name().to_string(),
                reason: format!("Invalid regex pattern: {e}"),
            })
    }

    fn build_glob(pattern: Option<&str>) -> Result<Option<GlobMatcher>> {
        pattern
            .map(|p| {
                Glob::new(p).map(|g| g.compile_matcher()).map_err(|e| {
                    AgentError::InvalidToolInput {
                        tool: ToolType::Grep.name().to_string(),
                        reason: format!("Invalid glob pattern: {e}"),
                    }
                })
            })
            .transpose()
    }

    fn resolve_search_path(path: Option<&str>) -> Result<PathBuf> {
        match path {
            Some(path_str) => {
                let path = validate_absolute_path(path_str, &ToolType::Grep)?;
                validate_path_exists(&path, &ToolType::Grep)?;
                Ok(path)
            }
            None => Ok(std::env::current_dir()?),
        }
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
        let config = SearchConfig {
            limit: input.limit.min(GREP_MAX_LIMIT),
            context: input.context.min(GREP_MAX_CONTEXT),
        };

        let matcher = Self::build_matcher(&input.pattern, input.ignore_case)?;
        let glob_matcher = Self::build_glob(input.glob.as_deref())?;
        let search_path = Self::resolve_search_path(input.path.as_deref())?;

        let mut searcher = SearcherBuilder::new()
            .binary_detection(BinaryDetection::quit(0x00))
            .line_number(true)
            .build();

        let mut collector = MatchCollector::with_capacity(config.limit);

        if search_path.is_file() {
            let () = search_file(
                &search_path,
                &matcher,
                &mut searcher,
                &config,
                &mut collector,
            );
        } else {
            let walker = walk_builder_with_gitignore(&search_path, input.respect_gitignore).build();

            for entry in walker.flatten() {
                let path = entry.path();

                if path.is_dir() || !matches_glob(path, glob_matcher.as_ref()) {
                    continue;
                }

                let () = search_file(path, &matcher, &mut searcher, &config, &mut collector);

                if collector.is_full() {
                    break;
                }
            }
        }

        let results = collector.into_vec();
        let output = SearchOutput {
            pattern: &input.pattern,
            results: &results,
            context: config.context,
            respect_gitignore: input.respect_gitignore,
        };

        Ok(output.to_string())
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
