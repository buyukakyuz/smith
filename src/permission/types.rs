use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PatternError {
    #[error("invalid glob pattern: {0}")]
    InvalidGlob(String),
    #[error("invalid regex pattern: {0}")]
    InvalidRegex(#[from] regex::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum PermissionType {
    FileRead,
    FileWrite,
    FileDelete,
    CommandExecute,
    NetworkAccess,
    SystemModification,
}

impl fmt::Display for PermissionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FileRead => write!(f, "read file"),
            Self::FileWrite => write!(f, "write file"),
            Self::FileDelete => write!(f, "delete file"),
            Self::CommandExecute => write!(f, "execute command"),
            Self::NetworkAccess => write!(f, "network access"),
            Self::SystemModification => write!(f, "system modification"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum PermissionResponse {
    AllowOnce,
    AllowSession,
    TellModelDifferently(String),
}

impl fmt::Display for PermissionResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AllowOnce => write!(f, "Allow once"),
            Self::AllowSession => write!(f, "Allow for session"),
            Self::TellModelDifferently(msg) => write!(f, "Tell model: {msg}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionCheckResult {
    Allowed,
    DeniedWithFeedback(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionRequest {
    pub operation_type: PermissionType,
    pub target: String,
    pub context: Option<String>,
}

impl PermissionRequest {
    #[must_use]
    pub fn new(operation_type: PermissionType, target: impl Into<String>) -> Self {
        Self {
            operation_type,
            target: target.into(),
            context: None,
        }
    }

    #[must_use]
    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "pattern")]
pub enum Pattern {
    Exact(String),
    Glob(String),
    Regex(String),
}

impl Pattern {
    pub fn matches(&self, target: &str) -> Result<bool, PatternError> {
        match self {
            Self::Exact(pattern) => Ok(target == pattern),
            Self::Glob(pattern) => glob::Pattern::new(pattern)
                .map(|p| p.matches(target))
                .map_err(|e| PatternError::InvalidGlob(e.to_string())),
            Self::Regex(pattern) => regex::Regex::new(pattern)
                .map(|re| re.is_match(target))
                .map_err(PatternError::InvalidRegex),
        }
    }
}

impl fmt::Display for Pattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Exact(s) | Self::Glob(s) => write!(f, "{s}"),
            Self::Regex(s) => write!(f, "/{s}/"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_type_display() {
        assert_eq!(PermissionType::FileRead.to_string(), "read file");
        assert_eq!(
            PermissionType::CommandExecute.to_string(),
            "execute command"
        );
    }

    #[test]
    fn test_permission_request_builder() {
        let request = PermissionRequest::new(PermissionType::FileWrite, "test.txt")
            .with_context("Testing file write");

        assert_eq!(request.operation_type, PermissionType::FileWrite);
        assert_eq!(request.target, "test.txt");
        assert_eq!(request.context, Some("Testing file write".to_string()));
    }

    #[test]
    fn test_pattern_exact_match() {
        let pattern = Pattern::Exact("test.txt".to_string());
        assert!(pattern.matches("test.txt").unwrap());
        assert!(!pattern.matches("other.txt").unwrap());
    }

    #[test]
    fn test_pattern_glob_match() {
        let pattern = Pattern::Glob("*.txt".to_string());
        assert!(pattern.matches("test.txt").unwrap());
        assert!(pattern.matches("other.txt").unwrap());
        assert!(!pattern.matches("test.rs").unwrap());
    }

    #[test]
    fn test_pattern_glob_recursive() {
        let pattern = Pattern::Glob("src/**/*.rs".to_string());
        assert!(pattern.matches("src/main.rs").unwrap());
        assert!(pattern.matches("src/lib.rs").unwrap());
        assert!(pattern.matches("src/core/mod.rs").unwrap());
        assert!(!pattern.matches("src/main.txt").unwrap());
    }

    #[test]
    fn test_pattern_regex_match() {
        let pattern = Pattern::Regex(r"^test_\w+\.rs$".to_string());
        assert!(pattern.matches("test_core.rs").unwrap());
        assert!(pattern.matches("test_tools.rs").unwrap());
        assert!(!pattern.matches("main.rs").unwrap());
    }
}
