use std::sync::Arc;

use super::error::Result;
use super::llm::LLM;
use super::memory::Memory;
use super::prompt::PromptBuilder;
use crate::permission::PermissionManager;
use crate::tools::{ToolContext, ToolEngine, ToolEventEmitter, ToolEventHandler, ToolRegistry};

mod config;
mod runner;
mod stream_accumulator;

pub use config::LoopConfig;
pub struct AugmentedLLM {
    llm: Arc<dyn LLM>,
    memory: Memory,
    tools: ToolRegistry,
    config: LoopConfig,
    permission_manager: Option<Arc<PermissionManager>>,
    tool_engine: ToolEngine,
}

impl AugmentedLLM {
    pub fn with_config(
        llm: Arc<dyn LLM>,
        config: LoopConfig,
        event_emitter: ToolEventEmitter,
    ) -> Result<Self> {
        let context = ToolContext::new()?;
        let tool_engine = ToolEngine::new(context, event_emitter);

        Ok(Self {
            llm,
            memory: Memory::new(),
            tools: ToolRegistry::new(),
            config,
            permission_manager: None,
            tool_engine,
        })
    }

    pub fn set_permission_manager(&mut self, manager: Arc<PermissionManager>) {
        self.permission_manager = Some(manager);
    }

    pub fn register_tool_event_handler(&mut self, handler: Arc<dyn ToolEventHandler>) {
        self.tool_engine.register_handler(handler);
    }

    pub fn set_system_prompt(&mut self, prompt: impl Into<String>) {
        self.memory.set_system_prompt(prompt);
    }

    pub fn set_llm(&mut self, llm: Arc<dyn LLM>) {
        self.llm = llm;
    }
    pub fn regenerate_system_prompt(&mut self) {
        use crate::core::prompt::TemplateType;

        let name = self.llm.name().to_lowercase();
        let template_type = if name.contains("gpt") || name.contains("openai") {
            TemplateType::OpenAI
        } else {
            TemplateType::Claude
        };

        let prompt = PromptBuilder::new()
            .with_template(template_type)
            .with_model(self.llm.name(), self.llm.model())
            .build(&self.tools);
        self.set_system_prompt(prompt);
    }

    #[must_use]
    pub const fn tools(&self) -> &ToolRegistry {
        &self.tools
    }

    pub const fn tools_mut(&mut self) -> &mut ToolRegistry {
        &mut self.tools
    }
    #[must_use]
    pub fn llm(&self) -> &dyn LLM {
        self.llm.as_ref()
    }
}
