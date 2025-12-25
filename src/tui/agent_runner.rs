use crate::config::{AppConfig, ModelInfo, ModelRegistry};
use crate::core::augmented_llm::AugmentedLLM;
use crate::core::error::AgentError;
use crate::permission::PermissionManager;
use crate::tui::TuiToolEventHandler;
use crate::tui::events::AppEvent;
use crate::tui::permission_ui::TuiPermissionUI;
use std::sync::Arc;
use tokio::sync::mpsc;

#[derive(Debug)]
pub enum AgentCommand {
    Run { user_message: String },
    SwitchModel { model_name: String },
    Shutdown,
}

#[derive(Clone)]
pub struct AgentConfig {
    pub model_id: Option<String>,
    pub max_iterations: Option<usize>,
    pub system_prompt: Option<String>,
    pub custom_system_prompt: Option<String>,
}

impl AgentConfig {
    #[must_use]
    pub fn from_model(model_id: Option<String>, config: &AppConfig) -> Self {
        Self {
            model_id,
            max_iterations: None,
            system_prompt: None,
            custom_system_prompt: config.custom_system_prompt.clone(),
        }
    }
}

pub struct AgentRunner {
    agent: Option<AugmentedLLM>,
    agent_config: AgentConfig,
    cmd_rx: mpsc::UnboundedReceiver<AgentCommand>,
    event_tx: mpsc::UnboundedSender<AppEvent>,
}

impl AgentRunner {
    #[must_use]
    pub fn new(
        agent_config: AgentConfig,
        event_tx: mpsc::UnboundedSender<AppEvent>,
    ) -> (Self, mpsc::UnboundedSender<AgentCommand>) {
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let runner = Self {
            agent: None,
            agent_config,
            cmd_rx,
            event_tx,
        };
        (runner, cmd_tx)
    }

    #[must_use]
    pub fn with_agent(
        agent: AugmentedLLM,
        event_tx: mpsc::UnboundedSender<AppEvent>,
    ) -> (Self, mpsc::UnboundedSender<AgentCommand>) {
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let runner = Self {
            agent: Some(agent),
            agent_config: AgentConfig {
                model_id: None,
                max_iterations: None,
                system_prompt: None,
                custom_system_prompt: None,
            },
            cmd_rx,
            event_tx,
        };
        (runner, cmd_tx)
    }

    pub async fn run(mut self) {
        while let Some(cmd) = self.cmd_rx.recv().await {
            match cmd {
                AgentCommand::Run { user_message } => {
                    if self.agent.is_none()
                        && let Err(e) = self.initialize_agent()
                    {
                        let _ = self.event_tx.send(AppEvent::LLMError(e.to_string()));
                        continue;
                    }
                    self.run_agent_with_events(user_message).await;
                }
                AgentCommand::SwitchModel { model_name } => {
                    self.switch_model(&model_name);
                }
                AgentCommand::Shutdown => {
                    tracing::info!("Agent runner shutting down");
                    break;
                }
            }
        }
    }

    fn initialize_agent(&mut self) -> Result<(), AgentError> {
        let registry = ModelRegistry::load();

        let model_info = if let Some(id) = &self.agent_config.model_id {
            registry
                .get_model(id)
                .ok_or_else(|| AgentError::Config(format!("Model '{id}' not found in registry")))?
        } else {
            registry
                .default_model()
                .ok_or_else(|| AgentError::Config("No default model configured".to_string()))?
        };

        self.create_agent_from_model(model_info)
    }

    fn create_agent_from_model(&mut self, model_info: &ModelInfo) -> Result<(), AgentError> {
        use crate::core::augmented_llm::LoopConfig;
        use crate::core::prompt::PromptBuilder;
        use crate::providers::factory::create_provider;
        use crate::tools::ToolEventEmitter;

        let llm = create_provider(model_info)?;

        let mut loop_config = LoopConfig::default();
        if let Some(max_iter) = self.agent_config.max_iterations {
            loop_config.max_iterations = max_iter;
        }
        let event_emitter = ToolEventEmitter::new();

        let mut agent = AugmentedLLM::with_config(llm.clone(), loop_config, event_emitter)?;

        let system_prompt = if let Some(prompt) = &self.agent_config.system_prompt {
            prompt.clone()
        } else {
            let template_type = Self::infer_template_type(&llm);
            let base_prompt = PromptBuilder::new()
                .with_template(template_type)
                .with_model(llm.name(), llm.model())
                .build(agent.tools());

            match self
                .agent_config
                .custom_system_prompt
                .as_deref()
                .filter(|s| !s.is_empty())
            {
                Some(custom) => format!("{base_prompt}\n\n# Custom Instructions\n\n{custom}"),
                None => base_prompt,
            }
        };
        agent.set_system_prompt(&system_prompt);

        let tools: Vec<Arc<dyn crate::tools::Tool>> = vec![
            Arc::new(crate::tools::ReadFileTool::new()),
            Arc::new(crate::tools::WriteFileTool::new()),
            Arc::new(crate::tools::UpdateFileTool::new()),
            Arc::new(crate::tools::ListDirTool::new()),
            Arc::new(crate::tools::GlobTool::new()),
            Arc::new(crate::tools::GrepTool::new()),
            Arc::new(crate::tools::BashTool::new()),
        ];
        for tool in tools {
            agent.tools_mut().register(tool);
        }

        agent
            .register_tool_event_handler(Arc::new(TuiToolEventHandler::new(self.event_tx.clone())));

        let permission_ui = Arc::new(TuiPermissionUI::new(self.event_tx.clone()));
        match PermissionManager::new(permission_ui) {
            Ok(pm) => {
                agent.set_permission_manager(Arc::new(pm));
            }
            Err(e) => {
                tracing::warn!("Failed to create permission manager: {e}");
            }
        }

        let provider = llm.name().to_string();
        let model = llm.model().to_string();
        let _ = self
            .event_tx
            .send(AppEvent::ModelChanged { provider, model });

        self.agent = Some(agent);
        Ok(())
    }

    fn infer_template_type(llm: &Arc<dyn crate::core::LLM>) -> crate::core::prompt::TemplateType {
        use crate::core::prompt::TemplateType;
        let name_lower = llm.name().to_lowercase();

        if name_lower.contains("gpt") || name_lower.contains("openai") {
            TemplateType::OpenAI
        } else if name_lower.contains("gemini") {
            TemplateType::Gemini
        } else {
            TemplateType::Claude
        }
    }

    fn switch_model(&mut self, model_id: &str) {
        let registry = ModelRegistry::load();

        let model_info = if let Some(info) = registry.get_model(model_id) {
            info
        } else {
            let _ = self.event_tx.send(AppEvent::ModelSwitchError(format!(
                "Model '{model_id}' not found"
            )));
            return;
        };

        self.agent_config.model_id = Some(model_id.to_string());

        if let Some(agent) = &mut self.agent {
            match crate::providers::factory::create_provider(model_info) {
                Ok(new_llm) => {
                    let provider = new_llm.name().to_string();
                    let model = new_llm.model().to_string();
                    agent.set_llm(new_llm);
                    agent.regenerate_system_prompt();
                    let _ = self
                        .event_tx
                        .send(AppEvent::ModelChanged { provider, model });
                }
                Err(e) => {
                    let _ = self
                        .event_tx
                        .send(AppEvent::ModelSwitchError(e.to_string()));
                }
            }
        } else if let Err(e) = self.create_agent_from_model(model_info) {
            let _ = self
                .event_tx
                .send(AppEvent::ModelSwitchError(e.to_string()));
        }
    }

    async fn run_agent_with_events(&mut self, message: String) {
        use crate::core::types::{ContentDelta, StreamEvent};

        let agent = if let Some(a) = &mut self.agent {
            a
        } else {
            let _ = self
                .event_tx
                .send(AppEvent::LLMError("Agent not initialized".to_string()));
            return;
        };

        let event_tx = self.event_tx.clone();

        let result = agent
            .run(message, |stream_event| {
                if let StreamEvent::ContentBlockDelta { delta, .. } = stream_event {
                    match delta {
                        ContentDelta::TextDelta { text } => {
                            let _ = event_tx.send(AppEvent::LLMChunk(text.clone()));
                        }
                        ContentDelta::ThinkingDelta { .. }
                        | ContentDelta::SignatureDelta { .. }
                        | ContentDelta::InputJsonDelta { .. } => {}
                    }
                }
            })
            .await;

        match result {
            Ok((message, usage)) => {
                let _ = self.event_tx.send(AppEvent::LLMComplete(message, usage));
            }
            Err(e) => {
                let _ = self.event_tx.send(AppEvent::LLMError(e.to_string()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::augmented_llm::LoopConfig;
    use crate::providers::mock::MockLLM;
    use crate::tools::events::ToolEventEmitter;

    #[test]
    fn test_agent_runner_construction_lazy() {
        let (event_tx, _event_rx) = mpsc::unbounded_channel();

        let config = AgentConfig {
            model_id: Some("claude-sonnet-4-5".to_string()),
            max_iterations: None,
            system_prompt: None,
            custom_system_prompt: None,
        };

        let (runner, _cmd_tx) = AgentRunner::new(config, event_tx);

        assert!(runner.agent.is_none());
        assert!(!runner.cmd_rx.is_closed());
    }

    #[test]
    fn test_agent_runner_construction_with_agent() {
        let (event_tx, _event_rx) = mpsc::unbounded_channel();

        let mock = MockLLM::new();
        let agent = AugmentedLLM::with_config(
            Arc::new(mock),
            LoopConfig::default(),
            ToolEventEmitter::new(),
        )
        .expect("Failed to create agent");

        let (runner, _cmd_tx) = AgentRunner::with_agent(agent, event_tx);

        assert!(runner.agent.is_some());
        assert!(!runner.cmd_rx.is_closed());
    }
}
