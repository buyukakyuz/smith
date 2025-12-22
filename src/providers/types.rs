#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::fmt;

use super::error::ProviderError;

#[derive(Clone)]
pub struct ApiKey(Cow<'static, str>);

impl ApiKey {
    #[must_use]
    pub fn new(key: impl Into<Cow<'static, str>>) -> Self {
        Self(key.into())
    }

    pub fn from_env(var_name: &str) -> Result<Self, ProviderError> {
        std::env::var(var_name)
            .map(|s| Self(Cow::Owned(s)))
            .map_err(|_| {
                ProviderError::Configuration(format!("Environment variable {var_name} not set"))
            })
    }

    #[must_use]
    pub fn from_env_or_empty(var_name: &str) -> Self {
        Self(Cow::Owned(std::env::var(var_name).unwrap_or_default()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let len = self.0.len();
        if len > 8 {
            write!(f, "ApiKey({}...{})", &self.0[..4], &self.0[len - 3..])
        } else if len > 0 {
            write!(f, "ApiKey(***)")
        } else {
            write!(f, "ApiKey(<empty>)")
        }
    }
}

impl Default for ApiKey {
    fn default() -> Self {
        Self(Cow::Borrowed(""))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ModelId(Cow<'static, str>);

impl ModelId {
    #[must_use]
    pub fn new(id: impl Into<Cow<'static, str>>) -> Self {
        Self(id.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for ModelId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for ModelId {
    fn default() -> Self {
        Self(Cow::Borrowed(""))
    }
}

impl AsRef<str> for ModelId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaseUrl(Cow<'static, str>);

impl BaseUrl {
    #[must_use]
    pub fn new(url: impl Into<Cow<'static, str>>) -> Self {
        let url = url.into();
        let url = if url.ends_with('/') {
            Cow::Owned(url.trim_end_matches('/').to_string())
        } else {
            url
        };
        Self(url)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn join(&self, path: &str) -> String {
        format!("{}{}", self.0, path)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for BaseUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for BaseUrl {
    fn default() -> Self {
        Self(Cow::Borrowed(""))
    }
}

impl AsRef<str> for BaseUrl {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_key_redacted_debug() {
        let key = ApiKey::new("sk-ant-api03-abcdefghijklmnop");
        let debug = format!("{key:?}");
        assert!(debug.contains("sk-a"));
        assert!(debug.contains("..."));
        assert!(debug.contains("nop"));
        assert!(!debug.contains("abcdefghijklmnop"));
    }

    #[test]
    fn test_api_key_short() {
        let key = ApiKey::new("short");
        let debug = format!("{key:?}");
        assert_eq!(debug, "ApiKey(***)");
    }

    #[test]
    fn test_api_key_empty() {
        let key = ApiKey::new("");
        assert!(key.is_empty());
        let debug = format!("{key:?}");
        assert_eq!(debug, "ApiKey(<empty>)");
    }

    #[test]
    fn test_model_id() {
        let model = ModelId::new("claude-sonnet-4-20250514");
        assert_eq!(model.as_str(), "claude-sonnet-4-20250514");
        assert_eq!(format!("{model}"), "claude-sonnet-4-20250514");
    }

    #[test]
    fn test_model_id_serialization() {
        let model = ModelId::new("gpt-4o");
        let json = serde_json::to_string(&model).expect("serialize");
        assert_eq!(json, "\"gpt-4o\"");

        let parsed: ModelId = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, model);
    }

    #[test]
    fn test_base_url_strips_trailing_slash() {
        let url = BaseUrl::new("https://api.openai.com/");
        assert_eq!(url.as_str(), "https://api.openai.com");
    }

    #[test]
    fn test_base_url_join() {
        let url = BaseUrl::new("https://api.anthropic.com");
        assert_eq!(
            url.join("/v1/messages"),
            "https://api.anthropic.com/v1/messages"
        );

        let url = BaseUrl::new("https://api.openai.com/");
        assert_eq!(
            url.join("/v1/responses"),
            "https://api.openai.com/v1/responses"
        );
    }

    #[test]
    fn test_base_url_multiple_trailing_slashes() {
        let url = BaseUrl::new("https://example.com///");
        assert_eq!(url.as_str(), "https://example.com");
    }

    #[test]
    fn test_api_key_from_env_missing() {
        let result = ApiKey::from_env("NONEXISTENT_VAR_12345");
        assert!(matches!(result, Err(ProviderError::Configuration(_))));
    }

    #[test]
    fn test_api_key_from_env_or_empty() {
        let key = ApiKey::from_env_or_empty("NONEXISTENT_VAR_12345");
        assert!(key.is_empty());
    }
}
