use crate::providers::types::{ApiKey, BaseUrl, ModelId};
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct ProviderCapabilities {
    pub vision: bool,
    pub tools: bool,
    pub streaming: bool,
    pub parallel_tool_calls: bool,
    pub json_mode: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ExtraHeaders {
    headers: HashMap<String, String>,
}

impl ExtraHeaders {
    pub fn new() -> Self {
        Self {
            headers: HashMap::new(),
        }
    }

    pub fn insert(&mut self, key: String, value: String) {
        self.headers.insert(key, value);
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &String)> {
        self.headers.iter()
    }
}

#[derive(Clone)]
pub enum OpenAICompatAuth {
    Bearer(ApiKey),
    CustomHeader { header_name: String, key: ApiKey },
    None,
}

#[derive(Clone)]
pub struct OpenAICompatConfig {
    pub base_url: BaseUrl,
    pub auth: OpenAICompatAuth,
    pub default_model: ModelId,
    pub provider_name: String,
    pub api_key_env_var: String,
    pub extra_headers: ExtraHeaders,
    pub capabilities: ProviderCapabilities,
    pub model_aliases: HashMap<String, String>,
}

impl OpenAICompatConfig {
    pub fn custom(provider_name: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            base_url: BaseUrl::from(base_url.into()),
            auth: OpenAICompatAuth::None,
            default_model: ModelId::from(String::new()),
            provider_name: provider_name.into(),
            api_key_env_var: String::new(),
            extra_headers: ExtraHeaders::new(),
            capabilities: ProviderCapabilities::default(),
            model_aliases: HashMap::new(),
        }
    }

    pub fn with_bearer_auth(mut self, key: ApiKey) -> Self {
        self.auth = OpenAICompatAuth::Bearer(key);
        self
    }

    pub fn with_default_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = ModelId::from(model.into());
        self
    }

    pub fn with_api_key_env_var(mut self, var: impl Into<String>) -> Self {
        self.api_key_env_var = var.into();
        self
    }

    pub fn with_extra_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.extra_headers.insert(key.into(), value.into());
        self
    }

    pub fn with_capabilities(mut self, caps: ProviderCapabilities) -> Self {
        self.capabilities = caps;
        self
    }

    pub fn with_model_alias(mut self, alias: impl Into<String>, model: impl Into<String>) -> Self {
        self.model_aliases.insert(alias.into(), model.into());
        self
    }

    pub fn resolve_model(&self, model: &str) -> String {
        self.model_aliases
            .get(model)
            .cloned()
            .unwrap_or_else(|| model.to_string())
    }

    pub fn openrouter(api_key: ApiKey) -> Self {
        let mut config = Self::custom("openrouter", "https://openrouter.ai/api")
            .with_bearer_auth(api_key)
            .with_api_key_env_var("OPENROUTER_API_KEY")
            .with_capabilities(ProviderCapabilities {
                vision: true,
                tools: true,
                streaming: true,
                parallel_tool_calls: true,
                json_mode: true,
            });

        config.extra_headers.insert(
            "HTTP-Referer".to_string(),
            "https://github.com/anthropics/smith".to_string(),
        );
        config
            .extra_headers
            .insert("X-Title".to_string(), "Smith CLI".to_string());

        config
    }

    pub fn together(api_key: ApiKey) -> Self {
        Self::custom("together", "https://api.together.xyz")
            .with_bearer_auth(api_key)
            .with_api_key_env_var("TOGETHER_API_KEY")
            .with_capabilities(ProviderCapabilities {
                vision: true,
                tools: true,
                streaming: true,
                parallel_tool_calls: true,
                json_mode: true,
            })
    }

    pub fn ollama() -> Self {
        Self::custom("ollama", "http://localhost:11434").with_capabilities(ProviderCapabilities {
            vision: true,
            tools: true,
            streaming: true,
            parallel_tool_calls: false,
            json_mode: true,
        })
    }

    pub fn groq(api_key: ApiKey) -> Self {
        Self::custom("groq", "https://api.groq.com/openai")
            .with_bearer_auth(api_key)
            .with_api_key_env_var("GROQ_API_KEY")
            .with_capabilities(ProviderCapabilities {
                vision: true,
                tools: true,
                streaming: true,
                parallel_tool_calls: true,
                json_mode: false,
            })
    }

    pub fn vllm(base_url: impl Into<String>) -> Self {
        Self::custom("vllm", base_url).with_capabilities(ProviderCapabilities {
            vision: false,
            tools: true,
            streaming: true,
            parallel_tool_calls: true,
            json_mode: false,
        })
    }

    pub fn azure(
        endpoint: impl Into<String>,
        api_key: ApiKey,
        deployment: impl Into<String>,
    ) -> Self {
        let mut config = Self::custom("azure", endpoint);
        config.auth = OpenAICompatAuth::CustomHeader {
            header_name: "api-key".to_string(),
            key: api_key,
        };
        config
            .extra_headers
            .insert("api-version".to_string(), "2024-02-15-preview".to_string());
        config.default_model = ModelId::from(deployment.into());
        config.api_key_env_var = "AZURE_OPENAI_API_KEY".to_string();
        config.capabilities = ProviderCapabilities {
            vision: true,
            tools: true,
            streaming: true,
            parallel_tool_calls: true,
            json_mode: true,
        };

        config
    }

    pub fn fireworks(api_key: ApiKey) -> Self {
        Self::custom("fireworks", "https://api.fireworks.ai/inference")
            .with_bearer_auth(api_key)
            .with_api_key_env_var("FIREWORKS_API_KEY")
            .with_capabilities(ProviderCapabilities {
                vision: true,
                tools: true,
                streaming: true,
                parallel_tool_calls: true,
                json_mode: true,
            })
    }
}
