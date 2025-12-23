use super::AugmentedLLM;
use super::stream_accumulator::StreamAccumulator;
use crate::core::error::{AgentError, Result};
use crate::core::types::{CompletionRequest, Message, Role, StreamEvent, Usage};
use crate::tools::ToolExecutor;
use futures::StreamExt;

impl AugmentedLLM {
    pub async fn run<F>(
        &mut self,
        user_message: impl Into<String>,
        mut on_event: F,
    ) -> Result<(Message, Usage)>
    where
        F: FnMut(&StreamEvent),
    {
        self.memory.push(Message::user(user_message));
        let mut total_usage = Usage::default();

        for _ in 0..self.config.max_iterations {
            let (assistant_message, turn_usage) = self.process_single_turn(&mut on_event).await?;
            if let Some(usage) = turn_usage {
                total_usage.add(&usage);
            }
            self.memory.push(assistant_message.clone());

            if !assistant_message.has_tool_use() {
                return Ok((assistant_message, total_usage));
            }

            self.execute_and_record_tools(&assistant_message).await;
        }

        Err(AgentError::MaxIterationsExceeded(
            self.config.max_iterations,
        ))
    }

    async fn process_single_turn<F>(&self, on_event: &mut F) -> Result<(Message, Option<Usage>)>
    where
        F: FnMut(&StreamEvent),
    {
        let request = self.build_completion_request();
        let mut stream = self.llm.stream(request).await?;
        let mut accumulator = StreamAccumulator::default();
        let mut final_usage: Option<Usage> = None;

        while let Some(event_result) = stream.next().await {
            let event = event_result?;
            on_event(&event);

            match event {
                StreamEvent::ContentBlockStart {
                    index,
                    content_block,
                } => {
                    accumulator.handle_block_start(index, content_block);
                }
                StreamEvent::ContentBlockDelta { index, delta } => {
                    accumulator.handle_delta(index, delta);
                }
                StreamEvent::MessageStart { usage, .. } => {
                    if let Some(u) = usage {
                        final_usage = Some(final_usage.map_or(u, |mut existing| {
                            existing.add(&u);
                            existing
                        }));
                    }
                }
                StreamEvent::MessageDelta { delta } => {
                    if let Some(u) = delta.usage {
                        final_usage = Some(final_usage.map_or(u, |mut existing| {
                            existing.add(&u);
                            existing
                        }));
                    }
                }
                StreamEvent::MessageStop => break,
                _ => {}
            }
        }

        Ok((
            Message::new(Role::Assistant, accumulator.into_content_blocks()),
            final_usage,
        ))
    }

    fn build_completion_request(&self) -> CompletionRequest {
        let mut request = CompletionRequest::new(self.memory.messages().to_vec())
            .with_max_tokens(self.config.max_tokens)
            .with_temperature(self.config.temperature);

        if let Some(prompt) = self.memory.system_prompt() {
            request = request.with_system_prompt(prompt);
        }

        if !self.tools.is_empty() {
            request = request.with_tools(self.tools.definitions());
        }

        request
    }

    async fn execute_and_record_tools(&mut self, assistant_message: &Message) {
        let executor = ToolExecutor::new(
            &self.tools,
            self.permission_manager.as_ref(),
            &self.tool_engine,
        );

        let tool_results = executor.execute_tools(assistant_message).await;

        for result in tool_results {
            self.memory.push(result);
        }
    }
}
