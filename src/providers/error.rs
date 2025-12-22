#![allow(dead_code)]

use std::time::Duration;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProviderError {
    #[error("Authentication failed: {message}")]
    Authentication {
        message: String,
        hint: Option<String>,
    },

    #[error("Rate limit exceeded: {message}")]
    RateLimit {
        message: String,
        retry_after: Option<Duration>,
    },

    #[error("Context window exceeded: {current} tokens exceeds {limit} limit")]
    ContextWindowExceeded { current: usize, limit: usize },

    #[error("Connection failed: {0}")]
    Connection(String),

    #[error("Request timed out after {0:?}")]
    Timeout(Duration),

    #[error("Server error ({status}): {message}")]
    Server { status: u16, message: String },

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Failed to parse response: {0}")]
    ParseError(String),

    #[error("Stream error: {0}")]
    StreamError(String),

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Model not found: {0}")]
    ModelNotFound(String),
}

impl ProviderError {
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::RateLimit { .. } | Self::Connection(_) | Self::Timeout(_) | Self::Server { .. }
        )
    }

    #[must_use]
    pub const fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::RateLimit { retry_after, .. } => *retry_after,
            Self::Server { .. } => Some(Duration::from_secs(1)),
            Self::Connection(_) | Self::Timeout(_) => Some(Duration::from_millis(500)),
            _ => None,
        }
    }

    #[must_use]
    pub fn auth_with_hint(message: impl Into<String>, hint: impl Into<String>) -> Self {
        Self::Authentication {
            message: message.into(),
            hint: Some(hint.into()),
        }
    }

    #[must_use]
    pub fn rate_limit(message: impl Into<String>) -> Self {
        Self::RateLimit {
            message: message.into(),
            retry_after: None,
        }
    }

    #[must_use]
    pub fn rate_limit_with_retry(message: impl Into<String>, retry_after: Duration) -> Self {
        Self::RateLimit {
            message: message.into(),
            retry_after: Some(retry_after),
        }
    }

    #[must_use]
    pub fn server(status: u16, message: impl Into<String>) -> Self {
        Self::Server {
            status,
            message: message.into(),
        }
    }

    #[must_use]
    pub fn from_status(status: u16, body: &str, api_key_env_var: &str) -> Self {
        let message = serde_json::from_str::<serde_json::Value>(body)
            .ok()
            .and_then(|v| v.get("error")?.get("message")?.as_str().map(String::from))
            .unwrap_or_else(|| format!("HTTP {status}"));

        match status {
            401 => Self::Authentication {
                message,
                hint: Some(format!("Check your {api_key_env_var} environment variable")),
            },
            429 => Self::RateLimit {
                message,
                retry_after: None,
            },
            400 if message.to_lowercase().contains("context")
                || message.to_lowercase().contains("token") =>
            {
                Self::ContextWindowExceeded {
                    current: 0,
                    limit: 0,
                }
            }
            400..=499 => Self::InvalidRequest(message),
            500..=599 => Self::Server { status, message },
            _ => Self::InvalidRequest(message),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_retryable() {
        assert!(ProviderError::rate_limit("too many requests").is_retryable());
        assert!(ProviderError::server(503, "overloaded").is_retryable());
        assert!(ProviderError::Connection("network error".into()).is_retryable());
        assert!(ProviderError::Timeout(Duration::from_secs(30)).is_retryable());

        assert!(!ProviderError::auth_with_hint("invalid", "check key").is_retryable());
        assert!(!ProviderError::InvalidRequest("bad request".into()).is_retryable());
        assert!(!ProviderError::Configuration("missing key".into()).is_retryable());
    }

    #[test]
    fn test_retry_after() {
        let rate_limit = ProviderError::rate_limit_with_retry("wait", Duration::from_secs(5));
        assert_eq!(rate_limit.retry_after(), Some(Duration::from_secs(5)));

        let server = ProviderError::server(500, "error");
        assert_eq!(server.retry_after(), Some(Duration::from_secs(1)));

        let auth = ProviderError::auth_with_hint("invalid", "check key");
        assert!(auth.retry_after().is_none());
    }

    #[test]
    fn test_from_status_401() {
        let body = r#"{"error": {"message": "Invalid API key"}}"#;
        let err = ProviderError::from_status(401, body, "ANTHROPIC_API_KEY");

        match err {
            ProviderError::Authentication { message, hint } => {
                assert_eq!(message, "Invalid API key");
                assert!(hint.unwrap().contains("ANTHROPIC_API_KEY"));
            }
            _ => panic!("Expected Authentication error"),
        }
    }

    #[test]
    fn test_from_status_429() {
        let body = r#"{"error": {"message": "Rate limit exceeded"}}"#;
        let err = ProviderError::from_status(429, body, "OPENAI_API_KEY");

        assert!(matches!(err, ProviderError::RateLimit { .. }));
    }

    #[test]
    fn test_from_status_500() {
        let body = r#"{"error": {"message": "Internal server error"}}"#;
        let err = ProviderError::from_status(500, body, "API_KEY");

        match err {
            ProviderError::Server { status, message } => {
                assert_eq!(status, 500);
                assert_eq!(message, "Internal server error");
            }
            _ => panic!("Expected Server error"),
        }
    }

    #[test]
    fn test_error_display() {
        let err = ProviderError::auth_with_hint("Invalid key", "Check env var");
        assert_eq!(err.to_string(), "Authentication failed: Invalid key");

        let err = ProviderError::rate_limit("Too many requests");
        assert_eq!(err.to_string(), "Rate limit exceeded: Too many requests");

        let err = ProviderError::ContextWindowExceeded {
            current: 100_000,
            limit: 128_000,
        };
        assert_eq!(
            err.to_string(),
            "Context window exceeded: 100000 tokens exceeds 128000 limit"
        );
    }
}
