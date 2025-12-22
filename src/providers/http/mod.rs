#![allow(dead_code)]

pub mod auth;
pub mod sse;

pub use auth::AuthStrategy;
pub use sse::SseParser;

use reqwest::Client;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::RetryTransientMiddleware;
use reqwest_retry::policies::ExponentialBackoff;
use std::time::Duration;

use crate::providers::error::ProviderError;

#[derive(Debug, Clone)]
pub struct HttpConfig {
    pub timeout: Duration,
    pub max_retries: u32,
    pub retry_min_delay: Duration,
    pub retry_max_delay: Duration,
    pub user_agent: Option<String>,
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(120),
            max_retries: 3,
            retry_min_delay: Duration::from_millis(500),
            retry_max_delay: Duration::from_secs(30),
            user_agent: None,
        }
    }
}

impl HttpConfig {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub const fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    #[must_use]
    pub const fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    #[must_use]
    pub fn with_user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = Some(user_agent.into());
        self
    }

    #[must_use]
    pub const fn without_retries(mut self) -> Self {
        self.max_retries = 0;
        self
    }
}

#[derive(Clone)]
pub struct HttpClient {
    inner: ClientWithMiddleware,
    #[allow(dead_code)]
    config: HttpConfig,
}

impl HttpClient {
    pub fn new() -> Result<Self, ProviderError> {
        Self::with_config(HttpConfig::default())
    }

    pub fn with_config(config: HttpConfig) -> Result<Self, ProviderError> {
        let retry_policy = ExponentialBackoff::builder()
            .retry_bounds(config.retry_min_delay, config.retry_max_delay)
            .build_with_max_retries(config.max_retries);

        let mut builder = Client::builder().timeout(config.timeout);

        if let Some(ref ua) = config.user_agent {
            builder = builder.user_agent(ua);
        }

        let client = builder.build().map_err(|e| {
            ProviderError::Configuration(format!("Failed to build HTTP client: {e}"))
        })?;

        let client_with_middleware = ClientBuilder::new(client)
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();

        Ok(Self {
            inner: client_with_middleware,
            config,
        })
    }

    #[must_use]
    pub fn post(&self, url: &str, auth: &AuthStrategy) -> reqwest_middleware::RequestBuilder {
        auth.apply(self.inner.post(url))
    }

    #[must_use]
    pub fn get(&self, url: &str, auth: &AuthStrategy) -> reqwest_middleware::RequestBuilder {
        auth.apply(self.inner.get(url))
    }

    #[must_use]
    pub fn delete(&self, url: &str, auth: &AuthStrategy) -> reqwest_middleware::RequestBuilder {
        auth.apply(self.inner.delete(url))
    }

    #[must_use]
    pub const fn inner(&self) -> &ClientWithMiddleware {
        &self.inner
    }
}

impl std::fmt::Debug for HttpClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpClient")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_config_defaults() {
        let config = HttpConfig::default();
        assert_eq!(config.timeout, Duration::from_secs(120));
        assert_eq!(config.max_retries, 3);
        assert!(config.user_agent.is_none());
    }

    #[test]
    fn test_http_config_builder() {
        let config = HttpConfig::new()
            .with_timeout(Duration::from_secs(60))
            .with_max_retries(5)
            .with_user_agent("smith/0.1.0");

        assert_eq!(config.timeout, Duration::from_secs(60));
        assert_eq!(config.max_retries, 5);
        assert_eq!(config.user_agent, Some("smith/0.1.0".to_string()));
    }

    #[test]
    fn test_http_config_without_retries() {
        let config = HttpConfig::new().without_retries();
        assert_eq!(config.max_retries, 0);
    }

    #[test]
    fn test_http_client_creation() {
        let client = HttpClient::new();
        assert!(client.is_ok());
    }

    #[test]
    fn test_http_client_with_config() {
        let config = HttpConfig::new()
            .with_timeout(Duration::from_secs(30))
            .with_user_agent("test-agent");

        let client = HttpClient::with_config(config);
        assert!(client.is_ok());
    }

    #[test]
    fn test_http_client_debug() {
        let client = HttpClient::new().expect("client");
        let debug = format!("{client:?}");
        assert!(debug.contains("HttpClient"));
    }
}
