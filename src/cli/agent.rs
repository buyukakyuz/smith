use std::sync::Arc;

use crate::config::AppConfig;
use crate::core::prompt::{PromptBuilder, TemplateType};
use crate::core::{AugmentedLLM, LLM, LoopConfig, Result};
use crate::tools::{self, ToolEventEmitter};

use super::Cli;

pub fn create_agent(llm: &Arc<dyn LLM>, cli: &Cli, config: &AppConfig) -> Result<AugmentedLLM> {
    let loop_config = LoopConfig {
        max_iterations: cli.max_iterations,
        ..Default::default()
    };
    let event_emitter = ToolEventEmitter::new();

    let mut agent = AugmentedLLM::with_config(llm.clone(), loop_config, event_emitter)?;

    let system_prompt = build_system_prompt(llm, cli, config, &agent);
    agent.set_system_prompt(&system_prompt);

    register_tools(&mut agent);

    Ok(agent)
}

fn build_system_prompt(
    llm: &Arc<dyn LLM>,
    cli: &Cli,
    config: &AppConfig,
    agent: &AugmentedLLM,
) -> String {
    if let Some(prompt) = &cli.system {
        return prompt.clone();
    }

    let template_type = infer_template_type(llm);

    let base_prompt = PromptBuilder::new()
        .with_template(template_type)
        .with_model(llm.name(), llm.model())
        .build(agent.tools());

    match config
        .custom_system_prompt
        .as_deref()
        .filter(|s| !s.is_empty())
    {
        Some(custom) => format!("{base_prompt}\n\n# Custom Instructions\n\n{custom}"),
        None => base_prompt,
    }
}

fn infer_template_type(llm: &Arc<dyn LLM>) -> TemplateType {
    let name_lower = llm.name().to_lowercase();

    if name_lower.contains("anthropic") || name_lower.contains("claude") {
        TemplateType::Claude
    } else if name_lower.contains("gemini") {
        TemplateType::Gemini
    } else {
        TemplateType::OpenAI
    }
}

fn register_tools(agent: &mut AugmentedLLM) {
    let tools: Vec<Arc<dyn tools::Tool>> = vec![
        Arc::new(tools::ReadFileTool::new()),
        Arc::new(tools::WriteFileTool::new()),
        Arc::new(tools::UpdateFileTool::new()),
        Arc::new(tools::ListDirTool::new()),
        Arc::new(tools::GlobTool::new()),
        Arc::new(tools::GrepTool::new()),
        Arc::new(tools::BashTool::new()),
    ];

    for tool in tools {
        agent.tools_mut().register(tool);
    }
}
