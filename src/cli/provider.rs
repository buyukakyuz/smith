use std::sync::Arc;

use crate::config::{AppConfig, ModelRegistry};
use crate::core::{AgentError, LLM, Result};
use crate::providers::factory::create_provider;

use super::Cli;

pub fn create_provider_for_cli(cli: &Cli, config: &AppConfig) -> Result<Arc<dyn LLM>> {
    let registry = ModelRegistry::load();

    let model_id = cli.model.as_ref().or(config.model.as_ref());

    let model_info = if let Some(id) = model_id {
        registry.get_model(id).ok_or_else(|| {
            AgentError::Config(format!(
                "Model '{}' not found in models.toml. Add it to your configuration.",
                id
            ))
        })?
    } else {
        registry.default_model().ok_or_else(|| {
            AgentError::Config(
                "No model specified and no default model configured. \
                 Use --model or set default: true in models.toml"
                    .to_string(),
            )
        })?
    };

    create_provider(model_info)
}
