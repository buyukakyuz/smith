#![allow(dead_code)]

mod convert;
mod types;

use async_trait::async_trait;
use futures::StreamExt;

use crate::core::error::Result;
use crate::core::llm::LLM;
use crate::core::types::{CompletionRequest, CompletionResponse, StreamResponse};
use crate::providers::error::ProviderError;
use crate::providers::http::{AuthStrategy, HttpClient, HttpConfig, SseParser};
use crate::providers::types::{ApiKey, BaseUrl, ModelId};

const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
const DEFAULT_MODEL: &str = "claude-sonnet-4-20250514";
const API_VERSION: &str = "2023-06-01";

#[derive(Clone)]
pub struct AnthropicProvider {
    http: HttpClient,
    auth: AuthStrategy,
    model: ModelId,
    base_url: BaseUrl,
}

impl std::fmt::Debug for AnthropicProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnthropicProvider")
            .field("model", &self.model)
            .field("base_url", &self.base_url)
            .finish_non_exhaustive()
    }
}

impl AnthropicProvider {
    pub fn new(api_key: ApiKey) -> std::result::Result<Self, ProviderError> {
        Ok(Self {
            http: HttpClient::new()?,
            auth: AuthStrategy::anthropic(api_key),
            model: ModelId::new(DEFAULT_MODEL),
            base_url: BaseUrl::new(DEFAULT_BASE_URL),
        })
    }

    pub fn from_env() -> std::result::Result<Self, ProviderError> {
        let api_key = ApiKey::from_env("ANTHROPIC_API_KEY")?;
        Self::new(api_key)
    }

    pub fn with_http_config(
        api_key: ApiKey,
        http_config: HttpConfig,
    ) -> std::result::Result<Self, ProviderError> {
        Ok(Self {
            http: HttpClient::with_config(http_config)?,
            auth: AuthStrategy::anthropic(api_key),
            model: ModelId::new(DEFAULT_MODEL),
            base_url: BaseUrl::new(DEFAULT_BASE_URL),
        })
    }

    #[must_use]
    pub fn with_model(mut self, model: impl Into<ModelId>) -> Self {
        self.model = model.into();
        self
    }

    #[must_use]
    pub fn with_base_url(mut self, base_url: impl Into<BaseUrl>) -> Self {
        self.base_url = base_url.into();
        self
    }

    fn parse_error(status: reqwest::StatusCode, body: &str) -> ProviderError {
        ProviderError::from_status(status.as_u16(), body, "ANTHROPIC_API_KEY")
    }
}

impl From<&str> for ModelId {
    fn from(s: &str) -> Self {
        Self::new(s.to_string())
    }
}

impl From<String> for ModelId {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

impl From<&String> for ModelId {
    fn from(s: &String) -> Self {
        Self::new(s.clone())
    }
}

impl From<&str> for BaseUrl {
    fn from(s: &str) -> Self {
        Self::new(s.to_string())
    }
}

impl From<String> for BaseUrl {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

#[async_trait]
impl LLM for AnthropicProvider {
    fn name(&self) -> &'static str {
        "anthropic"
    }

    fn model(&self) -> &str {
        self.model.as_str()
    }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        let api_request = convert::to_api_request(&self.model, &request);
        let url = self.base_url.join("/v1/messages");

        let body =
            serde_json::to_string(&api_request).map_err(crate::core::error::AgentError::Json)?;

        let response = self
            .http
            .post(&url, &self.auth)
            .header("anthropic-version", API_VERSION)
            .header("content-type", "application/json")
            .body(body)
            .send()
            .await
            .map_err(|e| ProviderError::Connection(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(Self::parse_error(status, &error_body).into());
        }

        let api_response: types::ApiResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::ParseError(e.to_string()))?;

        Ok(convert::from_api_response(api_response))
    }

    async fn stream(&self, request: CompletionRequest) -> Result<StreamResponse> {
        let mut api_request = convert::to_api_request(&self.model, &request);
        api_request.stream = Some(true);

        let url = self.base_url.join("/v1/messages");

        let body =
            serde_json::to_string(&api_request).map_err(crate::core::error::AgentError::Json)?;

        let response = self
            .http
            .post(&url, &self.auth)
            .header("anthropic-version", API_VERSION)
            .header("content-type", "application/json")
            .body(body)
            .send()
            .await
            .map_err(|e| ProviderError::Connection(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(Self::parse_error(status, &error_body).into());
        }

        let byte_stream = response.bytes_stream();
        let sse_stream = SseParser::parse_stream(byte_stream);

        let event_stream = sse_stream.filter_map(|result| async move {
            match result {
                Ok(sse_event) => convert::parse_stream_event(&sse_event.data).map(Ok),
                Err(e) => Some(Err(e.into())),
            }
        });

        Ok(Box::pin(event_stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_with_model() {
        let provider = AnthropicProvider::new(ApiKey::new("test-key"))
            .expect("create provider")
            .with_model("claude-3-opus");

        assert_eq!(provider.model(), "claude-3-opus");
    }

    #[test]
    fn test_provider_with_base_url() {
        let provider = AnthropicProvider::new(ApiKey::new("test-key"))
            .expect("create provider")
            .with_base_url("https://custom.api.com");

        assert_eq!(provider.base_url.as_str(), "https://custom.api.com");
    }

    #[test]
    fn test_provider_debug() {
        let provider = AnthropicProvider::new(ApiKey::new("secret-key")).expect("create provider");

        let debug = format!("{provider:?}");
        assert!(debug.contains("AnthropicProvider"));
        assert!(debug.contains("claude-sonnet-4"));
        assert!(!debug.contains("secret-key"));
    }

    #[test]
    fn test_parse_error_401() {
        let body = r#"{"error": {"message": "Invalid API key"}}"#;
        let err = AnthropicProvider::parse_error(reqwest::StatusCode::UNAUTHORIZED, body);

        assert!(matches!(err, ProviderError::Authentication { .. }));
    }

    #[test]
    fn test_parse_error_429() {
        let body = r#"{"error": {"message": "Rate limit exceeded"}}"#;
        let err = AnthropicProvider::parse_error(reqwest::StatusCode::TOO_MANY_REQUESTS, body);

        assert!(matches!(err, ProviderError::RateLimit { .. }));
    }

    #[test]
    fn test_parse_error_500() {
        let body = r#"{"error": {"message": "Internal server error"}}"#;
        let err = AnthropicProvider::parse_error(reqwest::StatusCode::INTERNAL_SERVER_ERROR, body);

        assert!(matches!(err, ProviderError::Server { .. }));
    }
}
