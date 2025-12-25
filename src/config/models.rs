use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

const DEFAULT_MODELS_TOML: &str = include_str!("models.toml");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum ProviderType {
    Anthropic,
    OpenAI,
    Gemini,
    OpenRouter,
    Together,
    Groq,
    Fireworks,
    Ollama,
    Vllm,
    Azure,
    Custom,
}

impl ProviderType {
    #[must_use]
    pub const fn default_api_key_env(&self) -> Option<&'static str> {
        match self {
            Self::Anthropic => Some("ANTHROPIC_API_KEY"),
            Self::OpenAI => Some("OPENAI_API_KEY"),
            Self::Gemini => Some("GEMINI_API_KEY"),
            Self::OpenRouter => Some("OPENROUTER_API_KEY"),
            Self::Together => Some("TOGETHER_API_KEY"),
            Self::Groq => Some("GROQ_API_KEY"),
            Self::Fireworks => Some("FIREWORKS_API_KEY"),
            Self::Ollama => None,
            Self::Vllm => None,
            Self::Azure => Some("AZURE_OPENAI_API_KEY"),
            Self::Custom => None,
        }
    }

    #[must_use]
    pub const fn default_base_url(&self) -> Option<&'static str> {
        match self {
            Self::Anthropic => Some("https://api.anthropic.com"),
            Self::OpenAI => Some("https://api.openai.com/v1"),
            Self::Gemini => Some("https://generativelanguage.googleapis.com/v1beta"),
            Self::OpenRouter => Some("https://openrouter.ai/api/v1"),
            Self::Together => Some("https://api.together.xyz/v1"),
            Self::Groq => Some("https://api.groq.com/openai/v1"),
            Self::Fireworks => Some("https://api.fireworks.ai/inference/v1"),
            Self::Ollama => Some("http://localhost:11434/v1"),
            Self::Vllm => Some("http://localhost:8000/v1"),
            Self::Azure => None,
            Self::Custom => None,
        }
    }

    #[must_use]
    pub const fn is_openai_compatible(&self) -> bool {
        matches!(
            self,
            Self::OpenRouter
                | Self::Together
                | Self::Groq
                | Self::Fireworks
                | Self::Ollama
                | Self::Vllm
                | Self::Azure
                | Self::Custom
        )
    }

    #[must_use]
    pub const fn display_name(&self) -> &'static str {
        match self {
            Self::Anthropic => "Anthropic",
            Self::OpenAI => "OpenAI",
            Self::Gemini => "Google Gemini",
            Self::OpenRouter => "OpenRouter",
            Self::Together => "Together AI",
            Self::Groq => "Groq",
            Self::Fireworks => "Fireworks AI",
            Self::Ollama => "Ollama",
            Self::Vllm => "vLLM",
            Self::Azure => "Azure OpenAI",
            Self::Custom => "Custom",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AuthType {
    #[default]
    Bearer,
    Header,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelProviderConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub auth_type: AuthType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_header: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extra_headers: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub provider: ProviderType,
    pub max_tokens: usize,
    #[serde(default)]
    pub default: bool,
    #[serde(default = "default_true")]
    pub supports_tools: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<ModelProviderConfig>,
}

const fn default_true() -> bool {
    true
}

impl ModelInfo {
    #[must_use]
    pub fn api_key_env(&self) -> Option<&str> {
        self.config
            .as_ref()
            .and_then(|c| c.api_key_env.as_deref())
            .or_else(|| self.provider.default_api_key_env())
    }

    #[must_use]
    pub fn base_url(&self) -> Option<&str> {
        self.config
            .as_ref()
            .and_then(|c| c.base_url.as_deref())
            .or_else(|| self.provider.default_base_url())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ModelsConfig {
    #[serde(default)]
    pub models: Vec<ModelInfo>,
}

pub struct ModelRegistry {
    models: Vec<ModelInfo>,
    by_id: HashMap<String, usize>,
    by_provider: HashMap<ProviderType, Vec<usize>>,
    default_model_idx: Option<usize>,
}

impl ModelRegistry {
    #[must_use]
    pub fn load() -> Self {
        if let Some(user_config) = Self::load_user_config() {
            return user_config;
        }

        Self::load_default()
    }

    #[must_use]
    pub fn load_default() -> Self {
        let config: ModelsConfig =
            toml::from_str(DEFAULT_MODELS_TOML).expect("Bundled models.toml should be valid");

        Self::from_config(config)
    }

    #[must_use]
    pub fn get_models_path() -> Option<PathBuf> {
        use super::get_config_dir;

        get_config_dir().map(|dir| dir.join("models.toml"))
    }

    #[must_use]
    pub fn get_model(&self, id: &str) -> Option<&ModelInfo> {
        self.by_id.get(id).map(|&idx| &self.models[idx])
    }

    #[must_use]
    pub fn default_model(&self) -> Option<&ModelInfo> {
        self.default_model_idx.map(|idx| &self.models[idx])
    }

    #[must_use]
    pub fn models_by_provider(&self) -> Vec<(ProviderType, Vec<&ModelInfo>)> {
        let mut result = Vec::new();

        let provider_order = [
            ProviderType::Anthropic,
            ProviderType::OpenAI,
            ProviderType::Gemini,
            ProviderType::OpenRouter,
            ProviderType::Together,
            ProviderType::Groq,
            ProviderType::Fireworks,
            ProviderType::Ollama,
            ProviderType::Vllm,
            ProviderType::Azure,
            ProviderType::Custom,
        ];

        for provider in provider_order {
            if let Some(indices) = self.by_provider.get(&provider)
                && !indices.is_empty()
            {
                let models = indices.iter().map(|&idx| &self.models[idx]).collect();
                result.push((provider, models));
            }
        }

        result
    }

    #[must_use]
    pub fn all_models(&self) -> &[ModelInfo] {
        &self.models
    }

    fn load_user_config() -> Option<Self> {
        let path = Self::get_models_path()?;
        if !path.exists() {
            return None;
        }

        let content = fs::read_to_string(&path).ok()?;
        let config: ModelsConfig = toml::from_str(&content).ok()?;

        Some(Self::from_config(config))
    }

    fn from_config(config: ModelsConfig) -> Self {
        let mut by_id = HashMap::new();
        let mut by_provider: HashMap<ProviderType, Vec<usize>> = HashMap::new();
        let mut default_model_idx = None;

        for (idx, model) in config.models.iter().enumerate() {
            by_id.insert(model.id.clone(), idx);

            by_provider
                .entry(model.provider.clone())
                .or_default()
                .push(idx);

            if model.default && default_model_idx.is_none() {
                default_model_idx = Some(idx);
            }
        }

        Self {
            models: config.models,
            by_id,
            by_provider,
            default_model_idx,
        }
    }
}
