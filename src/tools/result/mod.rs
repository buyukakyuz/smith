#[derive(Debug, Clone)]
pub enum ToolResult {
    Success {
        output: String,
    },
    Error {
        error: String,
        suggestions: Vec<String>,
    },
}

impl ToolResult {
    #[must_use]
    pub fn success(output: impl Into<String>) -> Self {
        Self::Success {
            output: output.into(),
        }
    }

    #[must_use]
    pub fn error(error: impl Into<String>) -> Self {
        Self::Error {
            error: error.into(),
            suggestions: Vec::new(),
        }
    }

    #[must_use]
    pub fn error_with_suggestions(error: impl Into<String>, suggestions: Vec<String>) -> Self {
        Self::Error {
            error: error.into(),
            suggestions,
        }
    }

    #[must_use]
    pub const fn is_success(&self) -> bool {
        matches!(self, Self::Success { .. })
    }

    #[must_use]
    pub fn output(&self) -> Option<&str> {
        match self {
            Self::Success { output, .. } => Some(output),
            Self::Error { .. } => None,
        }
    }

    #[must_use]
    pub fn to_llm_string(&self) -> String {
        match self {
            Self::Success { output, .. } => output.clone(),
            Self::Error { error, suggestions } => {
                if suggestions.is_empty() {
                    format!("Error: {error}")
                } else {
                    format!(
                        "Error: {error}\n\nSuggestions:\n{}",
                        suggestions
                            .iter()
                            .map(|s| format!("- {s}"))
                            .collect::<Vec<_>>()
                            .join("\n")
                    )
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_success_result() {
        let result = ToolResult::success("output");

        assert!(result.is_success());
        assert_eq!(result.output(), Some("output"));
        assert_eq!(result.to_llm_string(), "output");
    }

    #[test]
    fn test_error_result() {
        let result = ToolResult::error("something went wrong");

        assert!(!result.is_success());
        assert_eq!(result.output(), None);
        assert_eq!(result.to_llm_string(), "Error: something went wrong");
    }

    #[test]
    fn test_error_with_suggestions() {
        let suggestions = vec![
            "Try checking the file path".to_string(),
            "Ensure you have read permissions".to_string(),
        ];
        let result = ToolResult::error_with_suggestions("file not found", suggestions);

        assert!(!result.is_success());

        let llm_string = result.to_llm_string();
        assert!(llm_string.contains("Error: file not found"));
        assert!(llm_string.contains("Try checking the file path"));
        assert!(llm_string.contains("Ensure you have read permissions"));
    }
}
