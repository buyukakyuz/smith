use std::sync::Arc;

use crate::config::AppConfig;
use crate::core::{AgentError, LLM, Result};
use crate::providers::{AnthropicProvider, ApiKey, GeminiProvider, OpenAIClient};

use super::Cli;

fn infer_provider(model: &str) -> Option<&'static str> {
    match () {
        _ if model.starts_with("claude") => Some("anthropic"),
        _ if model.starts_with("gpt") || model.starts_with("o1") || model.starts_with("o3") => {
            Some("openai")
        }
        _ if model.starts_with("gemini") => Some("gemini"),
        _ => None,
    }
}

pub fn create_provider(cli: &Cli, config: &AppConfig) -> Result<Arc<dyn LLM>> {
    let model = cli.model.as_deref().or(config.model.as_deref());
    let provider_name = model.and_then(infer_provider).unwrap_or("anthropic");

    match provider_name {
        "anthropic" => create_anthropic(model),
        "openai" => create_openai(model),
        "gemini" => create_gemini(model),
        other => Err(AgentError::Config(format!(
            "Unknown provider: {other}. Use a model starting with 'claude', 'gpt', or 'gemini'"
        ))),
    }
}

fn create_anthropic(model: Option<&str>) -> Result<Arc<dyn LLM>> {
    let api_key =
        ApiKey::from_env("ANTHROPIC_API_KEY").map_err(|e| AgentError::Config(e.to_string()))?;

    let mut provider =
        AnthropicProvider::new(api_key).map_err(|e| AgentError::Config(e.to_string()))?;

    if let Some(m) = model {
        provider = provider.with_model(m);
    }

    Ok(Arc::new(provider))
}

fn create_openai(model: Option<&str>) -> Result<Arc<dyn LLM>> {
    let mut client = OpenAIClient::from_env()?;

    if let Some(m) = model {
        client = client.with_model(m);
    }

    Ok(Arc::new(client))
}

fn create_gemini(model: Option<&str>) -> Result<Arc<dyn LLM>> {
    let mut provider = GeminiProvider::from_env().map_err(|e| AgentError::Config(e.to_string()))?;

    if let Some(m) = model {
        provider = provider.with_model(m);
    }

    Ok(Arc::new(provider))
}
