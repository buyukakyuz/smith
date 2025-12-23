use super::result::ToolResult;
use super::types::ToolType;
use std::path::Path;

struct ErrorPattern {
    keywords: &'static [&'static str],
    suggestions: SuggestionSource,
}

enum SuggestionSource {
    Static(&'static [&'static str]),
    Dynamic(fn(&ErrorContext) -> Vec<String>),
}

pub struct ErrorContext<'a> {
    pub tool_name: &'a str,
    pub working_dir: &'a Path,
    pub default_timeout_ms: u64,
    pub max_output_size: usize,
}

impl ErrorPattern {
    const fn static_hints(
        keywords: &'static [&'static str],
        suggestions: &'static [&'static str],
    ) -> Self {
        Self {
            keywords,
            suggestions: SuggestionSource::Static(suggestions),
        }
    }

    const fn dynamic_hints(
        keywords: &'static [&'static str],
        generator: fn(&ErrorContext) -> Vec<String>,
    ) -> Self {
        Self {
            keywords,
            suggestions: SuggestionSource::Dynamic(generator),
        }
    }

    fn matches(&self, error_lower: &str) -> bool {
        self.keywords.iter().any(|kw| error_lower.contains(kw))
    }

    fn suggestions(&self, ctx: &ErrorContext) -> Vec<String> {
        match &self.suggestions {
            SuggestionSource::Static(s) => s.iter().map(|&s| s.to_string()).collect(),
            SuggestionSource::Dynamic(f) => f(ctx),
        }
    }
}

pub struct ErrorHintMatcher {
    patterns: Vec<ErrorPattern>,
}

impl Default for ErrorHintMatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl ErrorHintMatcher {
    #[must_use]
    pub fn new() -> Self {
        Self {
            patterns: vec![
                ErrorPattern::static_hints(
                    &["no such file", "not found"],
                    &[
                        "Verify the file path is correct",
                        "Check if the file exists in the expected location",
                        "Use list_dir to explore the directory",
                    ],
                ),
                ErrorPattern::static_hints(
                    &["permission denied", "access denied"],
                    &[
                        "Check file/directory permissions",
                        "Ensure you have the necessary access rights",
                        "Try using sudo if appropriate (for bash commands)",
                    ],
                ),
                ErrorPattern::dynamic_hints(&["not an absolute path", "must be absolute"], |ctx| {
                    vec![
                        "Use an absolute path instead of a relative path".to_string(),
                        format!("Current working directory: {}", ctx.working_dir.display()),
                    ]
                }),
                ErrorPattern::dynamic_hints(&["timeout", "timed out"], |ctx| {
                    vec![
                        format!(
                            "The operation exceeded the timeout limit ({}s)",
                            ctx.default_timeout_ms / 1000
                        ),
                        "Try breaking the operation into smaller steps".to_string(),
                        "Consider if the operation is hanging or stuck".to_string(),
                    ]
                }),
                ErrorPattern::dynamic_hints(&["file too large", "exceeds limit"], |ctx| {
                    vec![
                        format!("Maximum file size is {} bytes", ctx.max_output_size),
                        "Try reading the file in chunks or processing it differently".to_string(),
                    ]
                }),
                ErrorPattern::dynamic_hints(&["command not found", "not recognized"], |ctx| {
                    vec![
                        "Check if the command is installed and in PATH".to_string(),
                        "Verify the command spelling".to_string(),
                        if ctx.tool_name == ToolType::Bash.name() {
                            "Use which or whereis to locate the command".to_string()
                        } else {
                            "Try a different approach".to_string()
                        },
                    ]
                }),
                ErrorPattern::static_hints(
                    &["invalid path", "bad path"],
                    &[
                        "Ensure path is absolute (starts with /)",
                        "Check for invalid characters in path",
                    ],
                ),
                ErrorPattern::static_hints(
                    &["binary file", "not valid utf-8"],
                    &[
                        "Cannot read binary files as text",
                        "Use appropriate binary analysis tools",
                        "Try file command to identify file type",
                    ],
                ),
                ErrorPattern::static_hints(
                    &["not a directory"],
                    &[
                        "Path exists but is not a directory",
                        "Use read_file for files, list_dir for directories",
                    ],
                ),
                ErrorPattern::static_hints(
                    &["invalid pattern", "invalid glob", "glob"],
                    &[
                        "Check glob pattern syntax",
                        "Examples: *.rs, **/*.txt, src/**/*.{js,ts}",
                    ],
                ),
                ErrorPattern::static_hints(
                    &["regex", "invalid regular expression"],
                    &[
                        "Check regex pattern syntax",
                        r"Escape special characters: . * + ? [ ] ( ) { } ^ $ | \",
                        "Test regex at regex101.com",
                    ],
                ),
                ErrorPattern::static_hints(
                    &["parent directory"],
                    &[
                        "Parent directory does not exist",
                        "Use create_dirs: true to create parent directories",
                        "Or create the directory first using bash mkdir -p",
                    ],
                ),
            ],
        }
    }

    #[must_use]
    pub fn categorize(&self, ctx: &ErrorContext, error: &str) -> ToolResult {
        let error_lower = error.to_lowercase();

        for pattern in &self.patterns {
            if pattern.matches(&error_lower) {
                return ToolResult::error_with_suggestions(error, pattern.suggestions(ctx));
            }
        }

        ToolResult::error(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_context() -> ErrorContext<'static> {
        ErrorContext {
            tool_name: "test_tool",
            working_dir: Box::leak(Box::new(PathBuf::from("/tmp"))),
            default_timeout_ms: 120_000,
            max_output_size: 10 * 1024 * 1024,
        }
    }

    #[test]
    fn test_file_not_found_hints() {
        let matcher = ErrorHintMatcher::new();
        let ctx = test_context();
        let result = matcher.categorize(&ctx, "No such file or directory");

        assert!(!result.is_success());
        let output = result.to_llm_string();
        assert!(output.contains("Verify the file path"));
    }

    #[test]
    fn test_permission_denied_hints() {
        let matcher = ErrorHintMatcher::new();
        let ctx = test_context();
        let result = matcher.categorize(&ctx, "Permission denied");

        let output = result.to_llm_string();
        assert!(output.contains("permissions"));
    }

    #[test]
    fn test_timeout_hints_include_context() {
        let matcher = ErrorHintMatcher::new();
        let ctx = test_context();
        let result = matcher.categorize(&ctx, "Operation timed out");

        let output = result.to_llm_string();
        assert!(output.contains("120s"));
    }

    #[test]
    fn test_unknown_error_no_suggestions() {
        let matcher = ErrorHintMatcher::new();
        let ctx = test_context();
        let result = matcher.categorize(&ctx, "Something completely unknown happened");

        let output = result.to_llm_string();
        assert!(!output.contains("Suggestions"));
        assert!(output.contains("Something completely unknown"));
    }

    #[test]
    fn test_case_insensitive_matching() {
        let matcher = ErrorHintMatcher::new();
        let ctx = test_context();

        let result1 = matcher.categorize(&ctx, "NO SUCH FILE");
        let result2 = matcher.categorize(&ctx, "no such file");

        assert!(!result1.is_success());
        assert!(!result2.is_success());
    }
}
