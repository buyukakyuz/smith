use std::sync::Arc;

use crate::config::models::{AuthType, ModelInfo, ProviderType};
use crate::core::error::{AgentError, Result};
use crate::core::llm::LLM;

use super::anthropic::AnthropicProvider;
use super::gemini::GeminiProvider;
use super::openai::OpenAIProvider;
use super::openai_compat::{OpenAICompatAuth, OpenAICompatConfig, OpenAICompatProvider};
use super::types::ApiKey;

pub fn create_provider(model: &ModelInfo) -> Result<Arc<dyn LLM>> {
    match &model.provider {
        ProviderType::Anthropic => create_anthropic_provider(model),
        ProviderType::OpenAI => create_openai_provider(model),
        ProviderType::Gemini => create_gemini_provider(model),
        ProviderType::OpenRouter
        | ProviderType::Together
        | ProviderType::Groq
        | ProviderType::Fireworks
        | ProviderType::Ollama
        | ProviderType::Vllm
        | ProviderType::Azure
        | ProviderType::Custom => create_openai_compat_provider_internal(model),
    }
}

fn create_anthropic_provider(model: &ModelInfo) -> Result<Arc<dyn LLM>> {
    let api_key = get_api_key(model, "ANTHROPIC_API_KEY")?;

    let mut provider = AnthropicProvider::new(api_key)
        .map_err(|e| AgentError::Config(e.to_string()))?
        .with_model(&model.id);

    if let Some(base_url) = model.base_url() {
        provider = provider.with_base_url(base_url);
    }

    Ok(Arc::new(provider))
}

fn create_openai_provider(model: &ModelInfo) -> Result<Arc<dyn LLM>> {
    let api_key = get_api_key(model, "OPENAI_API_KEY")?;

    let mut provider = OpenAIProvider::new(api_key)
        .map_err(|e| AgentError::Config(e.to_string()))?
        .with_model(&model.id);

    if let Some(base_url) = model.base_url() {
        provider = provider.with_base_url(base_url);
    }

    Ok(Arc::new(provider))
}

fn create_gemini_provider(model: &ModelInfo) -> Result<Arc<dyn LLM>> {
    let api_key = get_api_key(model, "GEMINI_API_KEY")?;

    let mut provider = GeminiProvider::new(api_key)
        .map_err(|e| AgentError::Config(e.to_string()))?
        .with_model(&model.id);

    if let Some(base_url) = model.base_url() {
        provider = provider.with_base_url(base_url);
    }

    Ok(Arc::new(provider))
}

fn create_openai_compat_provider_internal(model: &ModelInfo) -> Result<Arc<dyn LLM>> {
    let config = build_openai_compat_config(model)?;

    let provider = OpenAICompatProvider::new(config)
        .map_err(|e| AgentError::Config(e.to_string()))?
        .with_model(&model.id);

    Ok(Arc::new(provider))
}

fn build_openai_compat_config(model: &ModelInfo) -> Result<OpenAICompatConfig> {
    match &model.provider {
        ProviderType::OpenRouter => {
            let api_key = get_api_key(model, "OPENROUTER_API_KEY")?;
            let mut config = OpenAICompatConfig::openrouter(api_key);
            apply_config_overrides(&mut config, model);
            Ok(config)
        }
        ProviderType::Together => {
            let api_key = get_api_key(model, "TOGETHER_API_KEY")?;
            let mut config = OpenAICompatConfig::together(api_key);
            apply_config_overrides(&mut config, model);
            Ok(config)
        }
        ProviderType::Groq => {
            let api_key = get_api_key(model, "GROQ_API_KEY")?;
            let mut config = OpenAICompatConfig::groq(api_key);
            apply_config_overrides(&mut config, model);
            Ok(config)
        }
        ProviderType::Fireworks => {
            let api_key = get_api_key(model, "FIREWORKS_API_KEY")?;
            let mut config = OpenAICompatConfig::fireworks(api_key);
            apply_config_overrides(&mut config, model);
            Ok(config)
        }
        ProviderType::Ollama => {
            let mut config = OpenAICompatConfig::ollama();
            apply_config_overrides(&mut config, model);
            Ok(config)
        }
        ProviderType::Vllm => {
            let base_url = model
                .base_url()
                .ok_or_else(|| {
                    AgentError::Config(
                        "vLLM provider requires base_url in model configuration".to_string(),
                    )
                })?
                .to_string();
            let mut config = OpenAICompatConfig::vllm(base_url);
            apply_config_overrides(&mut config, model);
            Ok(config)
        }
        ProviderType::Azure => {
            let base_url = model
                .base_url()
                .ok_or_else(|| {
                    AgentError::Config(
                        "Azure provider requires base_url (endpoint) in model configuration"
                            .to_string(),
                    )
                })?
                .to_string();
            let api_key = get_api_key(model, "AZURE_OPENAI_API_KEY")?;
            let deployment = model.id.clone();
            let mut config = OpenAICompatConfig::azure(base_url, api_key, deployment);
            apply_config_overrides(&mut config, model);
            Ok(config)
        }
        ProviderType::Custom => build_custom_config(model),
        _ => Err(AgentError::Config(format!(
            "Provider {} is not OpenAI-compatible",
            model.provider.display_name()
        ))),
    }
}

fn build_custom_config(model: &ModelInfo) -> Result<OpenAICompatConfig> {
    let base_url = model
        .base_url()
        .ok_or_else(|| {
            AgentError::Config(
                "Custom provider requires base_url in model configuration".to_string(),
            )
        })?
        .to_string();

    let mut config = OpenAICompatConfig::custom(&model.id, base_url);

    if let Some(model_config) = &model.config {
        match model_config.auth_type {
            AuthType::Bearer => {
                if let Some(api_key_env) = model.api_key_env() {
                    let api_key = ApiKey::from_env(api_key_env).map_err(|_| {
                        AgentError::Config(format!(
                            "API key not found. Set {api_key_env} environment variable for custom provider."
                        ))
                    })?;
                    config = config.with_bearer_auth(api_key);
                }
            }
            AuthType::Header => {
                if let Some(header_name) = &model_config.auth_header {
                    if let Some(api_key_env) = model.api_key_env() {
                        let api_key = ApiKey::from_env(api_key_env).map_err(|_| {
                            AgentError::Config(format!(
                                "API key not found. Set {api_key_env} environment variable for custom provider."
                            ))
                        })?;
                        config.auth = OpenAICompatAuth::CustomHeader {
                            header_name: header_name.clone(),
                            key: api_key,
                        };
                    }
                } else {
                    return Err(AgentError::Config(
                        "Custom provider with Header auth requires auth_header field".to_string(),
                    ));
                }
            }
            AuthType::None => {
                config.auth = OpenAICompatAuth::None;
            }
        }

        for (key, value) in &model_config.extra_headers {
            config = config.with_extra_header(key, value);
        }
    }

    apply_config_overrides(&mut config, model);
    Ok(config)
}

fn apply_config_overrides(config: &mut OpenAICompatConfig, model: &ModelInfo) {
    if let Some(model_config) = &model.config {
        if let Some(base_url) = &model_config.base_url {
            config.base_url = base_url.as_str().into();
        }

        for (key, value) in &model_config.extra_headers {
            config.extra_headers.insert(key.clone(), value.clone());
        }
    }

    if !model.supports_tools {
        config.capabilities.tools = false;
    }
}

fn get_api_key(model: &ModelInfo, default_env: &str) -> Result<ApiKey> {
    let env_var = model.api_key_env().unwrap_or(default_env);
    ApiKey::from_env(env_var).map_err(|_| {
        AgentError::Config(format!(
            "API key not found. Set {} environment variable for {} provider.",
            env_var,
            model.provider.display_name()
        ))
    })
}
