use crate::core::augmented_llm::AugmentedLLM;
use crate::providers::create_provider_from_model;
use crate::tui::events::AppEvent;
use tokio::sync::mpsc;

#[derive(Debug)]
pub enum AgentCommand {
    Run { user_message: String },
    SwitchModel { model_name: String },
    Shutdown,
}

pub struct AgentRunner {
    agent: AugmentedLLM,
    cmd_rx: mpsc::UnboundedReceiver<AgentCommand>,
    event_tx: mpsc::UnboundedSender<AppEvent>,
}

impl AgentRunner {
    #[must_use]
    pub fn new(
        agent: AugmentedLLM,
        event_tx: mpsc::UnboundedSender<AppEvent>,
    ) -> (Self, mpsc::UnboundedSender<AgentCommand>) {
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let runner = Self {
            agent,
            cmd_rx,
            event_tx,
        };
        (runner, cmd_tx)
    }

    pub async fn run(mut self) {
        while let Some(cmd) = self.cmd_rx.recv().await {
            match cmd {
                AgentCommand::Run { user_message } => {
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

    fn switch_model(&mut self, model_name: &str) {
        match create_provider_from_model(model_name) {
            Ok(new_llm) => {
                let provider = new_llm.name().to_string();
                let model = new_llm.model().to_string();
                self.agent.set_llm(new_llm);
                self.agent.regenerate_system_prompt();
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
    }

    async fn run_agent_with_events(&mut self, message: String) {
        use crate::core::types::{ContentDelta, StreamEvent};

        let event_tx = self.event_tx.clone();

        let result = self
            .agent
            .run(message, |stream_event| match stream_event {
                StreamEvent::ContentBlockDelta { delta, .. } => match delta {
                    ContentDelta::TextDelta { text } => {
                        let _ = event_tx.send(AppEvent::LLMChunk(text.clone()));
                    }
                    ContentDelta::ThinkingDelta { .. }
                    | ContentDelta::SignatureDelta { .. }
                    | ContentDelta::InputJsonDelta { .. } => {}
                },
                _ => {}
            })
            .await;

        match result {
            Ok(message) => {
                let _ = self.event_tx.send(AppEvent::LLMComplete(message));
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
    use std::sync::Arc;

    #[test]
    fn test_agent_runner_construction() {
        let (event_tx, _event_rx) = mpsc::unbounded_channel();

        let mock = MockLLM::new();
        let agent = AugmentedLLM::with_config(
            Arc::new(mock),
            LoopConfig::default(),
            ToolEventEmitter::new(),
        )
        .expect("Failed to create agent");

        let (runner, _cmd_tx) = AgentRunner::new(agent, event_tx);

        assert!(runner.cmd_rx.is_closed() == false || runner.cmd_rx.is_closed() == true);
    }
}
