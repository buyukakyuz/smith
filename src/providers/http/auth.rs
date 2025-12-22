use crate::providers::types::ApiKey;
use reqwest_middleware::RequestBuilder;

#[derive(Clone)]
pub enum AuthStrategy {
    Bearer(ApiKey),
    ApiKeyHeader {
        header_name: &'static str,
        key: ApiKey,
    },
    None,
}

impl AuthStrategy {
    #[must_use]
    pub const fn bearer(key: ApiKey) -> Self {
        Self::Bearer(key)
    }

    #[must_use]
    pub const fn anthropic(key: ApiKey) -> Self {
        Self::ApiKeyHeader {
            header_name: "x-api-key",
            key,
        }
    }

    #[must_use]
    pub const fn custom_header(header_name: &'static str, key: ApiKey) -> Self {
        Self::ApiKeyHeader { header_name, key }
    }

    #[must_use]
    pub fn apply(&self, request: RequestBuilder) -> RequestBuilder {
        match self {
            Self::Bearer(key) => {
                request.header("Authorization", format!("Bearer {}", key.as_str()))
            }
            Self::ApiKeyHeader { header_name, key } => request.header(*header_name, key.as_str()),
            Self::None => request,
        }
    }

    #[must_use]
    pub fn is_configured(&self) -> bool {
        match self {
            Self::Bearer(key) | Self::ApiKeyHeader { key, .. } => !key.is_empty(),
            Self::None => true,
        }
    }
}

impl std::fmt::Debug for AuthStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bearer(key) => f.debug_tuple("Bearer").field(key).finish(),
            Self::ApiKeyHeader { header_name, key } => f
                .debug_struct("ApiKeyHeader")
                .field("header_name", header_name)
                .field("key", key)
                .finish(),
            Self::None => write!(f, "None"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bearer_auth() {
        let auth = AuthStrategy::bearer(ApiKey::new("test-key"));
        assert!(auth.is_configured());
        let debug = format!("{auth:?}");
        assert!(debug.contains("Bearer"));
        assert!(!debug.contains("test-key"));
    }

    #[test]
    fn test_anthropic_auth() {
        let auth = AuthStrategy::anthropic(ApiKey::new("sk-ant-xxx"));
        assert!(auth.is_configured());

        if let AuthStrategy::ApiKeyHeader { header_name, .. } = auth {
            assert_eq!(header_name, "x-api-key");
        } else {
            panic!("Expected ApiKeyHeader variant");
        }
    }

    #[test]
    fn test_no_auth() {
        let auth = AuthStrategy::None;
        assert!(auth.is_configured());
    }

    #[test]
    fn test_empty_key_not_configured() {
        let auth = AuthStrategy::bearer(ApiKey::new(""));
        assert!(!auth.is_configured());
    }

    #[test]
    fn test_custom_header() {
        let auth = AuthStrategy::custom_header("X-Custom-Key", ApiKey::new("custom"));
        if let AuthStrategy::ApiKeyHeader { header_name, .. } = auth {
            assert_eq!(header_name, "X-Custom-Key");
        } else {
            panic!("Expected ApiKeyHeader variant");
        }
    }
}
