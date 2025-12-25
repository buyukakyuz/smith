#![allow(dead_code)]

pub mod config;
pub mod convert;
pub mod types;

use async_trait::async_trait;
use futures::StreamExt;
use std::sync::{Arc, Mutex};

use crate::core::error::Result;
use crate::core::llm::LLM;
use crate::core::types::{CompletionRequest, CompletionResponse, StreamResponse};
use crate::providers::error::ProviderError;
use crate::providers::http::{HttpClient, HttpConfig, SseParser};
use crate::providers::types::{ApiKey, BaseUrl, ModelId};

pub use config::{OpenAICompatAuth, OpenAICompatConfig, ProviderCapabilities};

#[derive(Clone)]
pub struct OpenAICompatProvider {
    http: HttpClient,
    config: OpenAICompatConfig,
    model: ModelId,
}

impl std::fmt::Debug for OpenAICompatProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenAICompatProvider")
            .field("provider", &self.config.provider_name)
            .field("model", &self.model)
            .field("base_url", &self.config.base_url)
            .finish_non_exhaustive()
    }
}

impl OpenAICompatProvider {
    pub fn new(config: OpenAICompatConfig) -> std::result::Result<Self, ProviderError> {
        let model = config.default_model.clone();
        Ok(Self {
            http: HttpClient::new()?,
            config,
            model,
        })
    }

    pub fn with_http_config(
        config: OpenAICompatConfig,
        http_config: HttpConfig,
    ) -> std::result::Result<Self, ProviderError> {
        let model = config.default_model.clone();
        Ok(Self {
            http: HttpClient::with_config(http_config)?,
            config,
            model,
        })
    }

    pub fn openrouter(api_key: ApiKey) -> std::result::Result<Self, ProviderError> {
        Self::new(OpenAICompatConfig::openrouter(api_key))
    }

    pub fn openrouter_from_env() -> std::result::Result<Self, ProviderError> {
        let api_key = ApiKey::from_env("OPENROUTER_API_KEY")?;
        Self::openrouter(api_key)
    }

    pub fn together(api_key: ApiKey) -> std::result::Result<Self, ProviderError> {
        Self::new(OpenAICompatConfig::together(api_key))
    }

    pub fn together_from_env() -> std::result::Result<Self, ProviderError> {
        let api_key = ApiKey::from_env("TOGETHER_API_KEY")?;
        Self::together(api_key)
    }

    pub fn ollama() -> std::result::Result<Self, ProviderError> {
        Self::new(OpenAICompatConfig::ollama())
    }

    pub fn groq(api_key: ApiKey) -> std::result::Result<Self, ProviderError> {
        Self::new(OpenAICompatConfig::groq(api_key))
    }

    pub fn groq_from_env() -> std::result::Result<Self, ProviderError> {
        let api_key = ApiKey::from_env("GROQ_API_KEY")?;
        Self::groq(api_key)
    }

    pub fn vllm(base_url: impl Into<BaseUrl>) -> std::result::Result<Self, ProviderError> {
        Self::new(OpenAICompatConfig::vllm(
            base_url.into().as_str().to_string(),
        ))
    }

    pub fn azure(
        endpoint: impl Into<BaseUrl>,
        api_key: ApiKey,
        deployment: impl Into<String>,
    ) -> std::result::Result<Self, ProviderError> {
        Self::new(OpenAICompatConfig::azure(
            endpoint.into().as_str().to_string(),
            api_key,
            deployment,
        ))
    }

    pub fn fireworks(api_key: ApiKey) -> std::result::Result<Self, ProviderError> {
        Self::new(OpenAICompatConfig::fireworks(api_key))
    }

    pub fn fireworks_from_env() -> std::result::Result<Self, ProviderError> {
        let api_key = ApiKey::from_env("FIREWORKS_API_KEY")?;
        Self::fireworks(api_key)
    }

    #[must_use]
    pub fn with_model(mut self, model: impl Into<ModelId>) -> Self {
        self.model = model.into();
        self
    }

    #[must_use]
    pub fn with_base_url(mut self, base_url: impl Into<BaseUrl>) -> Self {
        self.config.base_url = base_url.into();
        self
    }

    fn endpoint(&self) -> String {
        self.config.base_url.join("/v1/chat/completions")
    }

    fn apply_auth(
        &self,
        builder: reqwest_middleware::RequestBuilder,
    ) -> reqwest_middleware::RequestBuilder {
        match &self.config.auth {
            OpenAICompatAuth::Bearer(key) => {
                builder.header("Authorization", format!("Bearer {}", key.as_str()))
            }
            OpenAICompatAuth::CustomHeader { header_name, key } => {
                builder.header(header_name.as_str(), key.as_str())
            }
            OpenAICompatAuth::None => builder,
        }
    }

    fn apply_extra_headers(
        &self,
        mut builder: reqwest_middleware::RequestBuilder,
    ) -> reqwest_middleware::RequestBuilder {
        for (key, value) in self.config.extra_headers.iter() {
            builder = builder.header(key.as_str(), value.as_str());
        }
        builder
    }

    fn parse_error(&self, status: reqwest::StatusCode, body: &str) -> ProviderError {
        if let Ok(api_error) = serde_json::from_str::<types::ApiError>(body) {
            let message = api_error.error.message;
            let hint = if self.config.api_key_env_var.is_empty() {
                None
            } else {
                Some(format!(
                    "Check your {} environment variable",
                    self.config.api_key_env_var
                ))
            };

            return match status.as_u16() {
                401 => ProviderError::Authentication { message, hint },
                429 => ProviderError::RateLimit {
                    message,
                    retry_after: None,
                },
                400 if message.to_lowercase().contains("context")
                    || message.to_lowercase().contains("token") =>
                {
                    ProviderError::ContextWindowExceeded {
                        current: 0,
                        limit: 0,
                    }
                }
                400..=499 => ProviderError::InvalidRequest(message),
                500..=599 => ProviderError::Server {
                    status: status.as_u16(),
                    message,
                },
                _ => ProviderError::InvalidRequest(message),
            };
        }

        ProviderError::from_status(status.as_u16(), body, &self.config.api_key_env_var)
    }
}

#[async_trait]
impl LLM for OpenAICompatProvider {
    fn name(&self) -> &'static str {
        match self.config.provider_name.as_str() {
            "openrouter" => "openrouter",
            "together" => "together",
            "ollama" => "ollama",
            "groq" => "groq",
            "vllm" => "vllm",
            "azure" => "azure",
            "fireworks" => "fireworks",
            _ => "openai_compat",
        }
    }

    fn model(&self) -> &str {
        self.model.as_str()
    }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        let mut api_request = convert::to_api_request(&self.config, &request);
        api_request.model = self.config.resolve_model(self.model.as_str());

        let url = self.endpoint();

        let body =
            serde_json::to_string(&api_request).map_err(crate::core::error::AgentError::Json)?;

        let builder = self.http.inner().post(&url);
        let builder = self.apply_auth(builder);
        let builder = self.apply_extra_headers(builder);
        let builder = builder
            .header("content-type", "application/json")
            .body(body);

        let response = builder
            .send()
            .await
            .map_err(|e| ProviderError::Connection(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(self.parse_error(status, &error_body).into());
        }

        let api_response: types::ChatCompletionResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::ParseError(e.to_string()))?;

        Ok(convert::from_api_response(api_response))
    }

    async fn stream(&self, request: CompletionRequest) -> Result<StreamResponse> {
        let mut api_request = convert::to_api_request(&self.config, &request);
        api_request.model = self.config.resolve_model(self.model.as_str());
        api_request.stream = Some(true);

        let url = self.endpoint();

        let body =
            serde_json::to_string(&api_request).map_err(crate::core::error::AgentError::Json)?;

        let builder = self.http.inner().post(&url);
        let builder = self.apply_auth(builder);
        let builder = self.apply_extra_headers(builder);
        let builder = builder
            .header("content-type", "application/json")
            .body(body);

        let response = builder
            .send()
            .await
            .map_err(|e| ProviderError::Connection(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(self.parse_error(status, &error_body).into());
        }

        let byte_stream = response.bytes_stream();
        let sse_stream = SseParser::parse_stream(byte_stream);

        let state = Arc::new(Mutex::new(convert::StreamState::new()));

        let event_stream = sse_stream.filter_map(move |result| {
            let state = state.clone();
            async move {
                match result {
                    Ok(sse_event) => {
                        let mut state_guard = state.lock().ok()?;
                        convert::parse_stream_event(&sse_event.data, &mut state_guard).map(Ok)
                    }
                    Err(e) => Some(Err(e.into())),
                }
            }
        });

        Ok(Box::pin(event_stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_creation() {
        let provider = OpenAICompatProvider::ollama();
        assert!(provider.is_ok());

        let provider = provider.unwrap();
        assert_eq!(provider.name(), "ollama");
    }

    #[test]
    fn test_provider_with_model() {
        let provider = OpenAICompatProvider::ollama()
            .expect("create provider")
            .with_model("mistral");

        assert_eq!(provider.model(), "mistral");
    }

    #[test]
    fn test_provider_debug_hides_secrets() {
        let config = OpenAICompatConfig::openrouter(ApiKey::new("secret-key-12345"));
        let provider = OpenAICompatProvider::new(config).expect("create provider");

        let debug = format!("{provider:?}");
        assert!(debug.contains("OpenAICompatProvider"));
        assert!(debug.contains("openrouter"));
        assert!(!debug.contains("secret-key-12345"));
    }

    #[test]
    fn test_endpoint_generation() {
        let provider = OpenAICompatProvider::ollama().expect("create provider");
        assert_eq!(
            provider.endpoint(),
            "http://localhost:11434/v1/chat/completions"
        );
    }

    #[test]
    fn test_openrouter_creation() {
        let config = OpenAICompatConfig::openrouter(ApiKey::new("test-key"));
        let provider = OpenAICompatProvider::new(config).expect("create provider");

        assert_eq!(provider.name(), "openrouter");
        assert_eq!(
            provider.endpoint(),
            "https://openrouter.ai/api/v1/chat/completions"
        );
    }

    #[test]
    fn test_groq_creation() {
        let config = OpenAICompatConfig::groq(ApiKey::new("test-key"));
        let provider = OpenAICompatProvider::new(config).expect("create provider");

        assert_eq!(provider.name(), "groq");
        assert_eq!(
            provider.endpoint(),
            "https://api.groq.com/openai/v1/chat/completions"
        );
    }

    #[test]
    fn test_together_creation() {
        let config = OpenAICompatConfig::together(ApiKey::new("test-key"));
        let provider = OpenAICompatProvider::new(config).expect("create provider");

        assert_eq!(provider.name(), "together");
    }

    #[test]
    fn test_fireworks_creation() {
        let config = OpenAICompatConfig::fireworks(ApiKey::new("test-key"));
        let provider = OpenAICompatProvider::new(config).expect("create provider");

        assert_eq!(provider.name(), "fireworks");
    }

    #[test]
    fn test_azure_creation() {
        let config = OpenAICompatConfig::azure(
            "https://my-resource.openai.azure.com",
            ApiKey::new("test-key"),
            "gpt-4",
        );
        let provider = OpenAICompatProvider::new(config).expect("create provider");

        assert_eq!(provider.name(), "azure");
        assert_eq!(provider.model(), "gpt-4");
    }

    #[test]
    fn test_vllm_creation() {
        let config = OpenAICompatConfig::vllm("http://localhost:8000");
        let provider = OpenAICompatProvider::new(config).expect("create provider");

        assert_eq!(provider.name(), "vllm");
        assert_eq!(
            provider.endpoint(),
            "http://localhost:8000/v1/chat/completions"
        );
    }

    #[test]
    fn test_custom_provider() {
        let config = OpenAICompatConfig::custom("my-provider", "https://api.example.com")
            .with_bearer_auth(ApiKey::new("test-key"))
            .with_default_model("my-model")
            .with_extra_header("X-Custom", "value")
            .with_capabilities(ProviderCapabilities {
                vision: true,
                tools: true,
                streaming: true,
                parallel_tool_calls: false,
                json_mode: false,
            });

        let provider = OpenAICompatProvider::new(config).expect("create provider");

        assert_eq!(provider.name(), "openai_compat");
        assert_eq!(provider.model(), "my-model");
    }
}
