use std::sync::Arc;

use crate::core::error::{AgentError, Result};
use crate::core::llm::LLM;

use super::anthropic::AnthropicProvider;
use super::openai::OpenAIProvider;
use super::types::ApiKey;

#[must_use]
pub fn infer_provider(model: &str) -> Option<&'static str> {
    if model.starts_with("claude") {
        Some("anthropic")
    } else if model.starts_with("gpt") || model.starts_with("o1") || model.starts_with("o3") {
        Some("openai")
    } else {
        None
    }
}

pub fn create_provider_from_model(model: &str) -> Result<Arc<dyn LLM>> {
    let provider_type = infer_provider(model).ok_or_else(|| {
        AgentError::Config(format!(
            "Unknown model: {model}. Use a model starting with 'claude', 'gpt', 'o1', or 'o3'."
        ))
    })?;

    match provider_type {
        "openai" => {
            let client = OpenAIProvider::from_env()
                .map_err(|e| AgentError::Config(e.to_string()))?
                .with_model(model);
            Ok(Arc::new(client))
        }
        "anthropic" => {
            let api_key = ApiKey::from_env("ANTHROPIC_API_KEY")
                .map_err(|e| AgentError::Config(e.to_string()))?;
            let provider = AnthropicProvider::new(api_key)
                .map_err(|e| AgentError::Config(e.to_string()))?
                .with_model(model);
            Ok(Arc::new(provider))
        }
        _ => unreachable!(),
    }
}
