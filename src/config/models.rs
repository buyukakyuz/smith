use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

const DEFAULT_MODELS_TOML: &str = include_str!("models.toml");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub max_tokens: usize,
    #[serde(default)]
    pub default: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ModelsConfig {
    #[serde(default)]
    pub anthropic: Vec<ModelInfo>,
    #[serde(default)]
    pub openai: Vec<ModelInfo>,
    #[serde(default)]
    pub gemini: Vec<ModelInfo>,
    #[serde(default)]
    pub custom: Vec<ModelInfo>,
}
pub struct ModelRegistry {
    models_by_provider: HashMap<String, Vec<ModelInfo>>,
}

impl ModelRegistry {
    #[must_use]
    pub fn load() -> Self {
        if let Some(user_config) = Self::load_user_config() {
            return user_config;
        }

        Self::load_default()
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

    #[must_use]
    pub fn load_default() -> Self {
        let config: ModelsConfig =
            toml::from_str(DEFAULT_MODELS_TOML).expect("Bundled models.toml should be valid");

        Self::from_config(config)
    }

    fn from_config(config: ModelsConfig) -> Self {
        let mut models_by_provider = HashMap::new();

        if !config.anthropic.is_empty() {
            models_by_provider.insert("anthropic".to_string(), config.anthropic);
        }

        if !config.openai.is_empty() {
            models_by_provider.insert("openai".to_string(), config.openai);
        }

        if !config.gemini.is_empty() {
            models_by_provider.insert("gemini".to_string(), config.gemini);
        }

        if !config.custom.is_empty() {
            models_by_provider.insert("custom".to_string(), config.custom);
        }

        Self { models_by_provider }
    }

    #[must_use]
    pub fn get_models_path() -> Option<PathBuf> {
        use super::get_config_dir;

        get_config_dir().map(|dir| dir.join("models.toml"))
    }
    #[must_use]
    pub fn all_models_by_provider(&self) -> Vec<(&str, &[ModelInfo])> {
        let mut result = Vec::new();
        for provider in ["anthropic", "openai", "gemini", "custom"] {
            if let Some(models) = self.models_by_provider.get(provider) {
                if !models.is_empty() {
                    result.push((provider, models.as_slice()));
                }
            }
        }
        result
    }
}
