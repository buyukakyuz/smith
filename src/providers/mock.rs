#![allow(clippy::expect_used, dead_code)]

use async_trait::async_trait;
use std::sync::{Arc, Mutex};

use crate::core::error::{AgentError, Result};
use crate::core::llm::LLM;
use crate::core::types::{
    CompletionRequest, CompletionResponse, ContentBlock, Message, Role, StopReason, StreamResponse,
    Usage,
};

#[derive(Debug, Clone)]
pub struct MockResponse {
    pub content: Vec<ContentBlock>,
    pub stop_reason: StopReason,
    pub usage: Usage,
}

impl MockResponse {
    #[must_use]
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: vec![ContentBlock::text(text)],
            stop_reason: StopReason::EndTurn,
            usage: Usage::new(10, 5),
        }
    }

    #[must_use]
    pub fn tool_use(name: impl Into<String>, input: serde_json::Value) -> Self {
        Self {
            content: vec![ContentBlock::tool_use(name, input)],
            stop_reason: StopReason::ToolUse,
            usage: Usage::new(10, 5),
        }
    }

    #[must_use]
    pub const fn multi(content: Vec<ContentBlock>, stop_reason: StopReason) -> Self {
        Self {
            content,
            stop_reason,
            usage: Usage::new(10, 5),
        }
    }

    #[must_use]
    pub const fn with_usage(mut self, usage: Usage) -> Self {
        self.usage = usage;
        self
    }
}

#[derive(Clone)]
pub struct MockLLM {
    name: String,
    model: String,
    responses: Arc<Mutex<Vec<MockResponse>>>,
    request_history: Arc<Mutex<Vec<CompletionRequest>>>,
}

impl MockLLM {
    #[must_use]
    pub fn new() -> Self {
        Self {
            name: "mock".to_string(),
            model: "mock-model".to_string(),
            responses: Arc::new(Mutex::new(Vec::new())),
            request_history: Arc::new(Mutex::new(Vec::new())),
        }
    }

    #[must_use]
    pub fn with_response(self, response: MockResponse) -> Self {
        self.responses
            .lock()
            .expect("MockLLM mutex poisoned")
            .push(response);
        self
    }

    #[must_use]
    pub fn with_default_response(self) -> Self {
        self.with_response(MockResponse::text("Mock response"))
    }

    #[must_use]
    pub fn request_history(&self) -> Vec<CompletionRequest> {
        self.request_history
            .lock()
            .expect("MockLLM mutex poisoned")
            .clone()
    }

    #[must_use]
    pub fn request_count(&self) -> usize {
        self.request_history
            .lock()
            .expect("MockLLM mutex poisoned")
            .len()
    }

    pub fn clear_history(&self) {
        self.request_history
            .lock()
            .expect("MockLLM mutex poisoned")
            .clear();
    }

    fn next_response(&self) -> Result<MockResponse> {
        let mut responses = self.responses.lock().expect("MockLLM mutex poisoned");
        if responses.is_empty() {
            Err(AgentError::Provider(
                "MockLLM: No responses queued".to_string(),
            ))
        } else {
            Ok(responses.remove(0))
        }
    }
}

impl Default for MockLLM {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LLM for MockLLM {
    fn name(&self) -> &str {
        &self.name
    }

    fn model(&self) -> &str {
        &self.model
    }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        self.request_history
            .lock()
            .expect("MockLLM mutex poisoned")
            .push(request);

        let response = self.next_response()?;

        let message = Message {
            role: Role::Assistant,
            content: response.content,
        };

        Ok(CompletionResponse::new(
            message,
            response.stop_reason,
            response.usage,
        ))
    }

    async fn stream(&self, request: CompletionRequest) -> Result<StreamResponse> {
        use futures::stream;

        self.request_history
            .lock()
            .expect("MockLLM mutex poisoned")
            .push(request);
        let _response = self.next_response()?;

        Ok(Box::pin(stream::empty()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_llm_returns_queued_response() {
        let mock = MockLLM::new()
            .with_response(MockResponse::text("First response"))
            .with_response(MockResponse::text("Second response"));

        let request = CompletionRequest::new(vec![Message::user("test")]);

        let response1 = mock.complete(request.clone()).await.unwrap();
        assert_eq!(response1.message.first_text(), Some("First response"));

        let response2 = mock.complete(request).await.unwrap();
        assert_eq!(response2.message.first_text(), Some("Second response"));
    }

    #[tokio::test]
    async fn test_mock_llm_error_when_empty() {
        let mock = MockLLM::new();
        let request = CompletionRequest::new(vec![Message::user("test")]);
        let result = mock.complete(request).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mock_llm_records_requests() {
        let mock = MockLLM::new().with_default_response();

        assert_eq!(mock.request_count(), 0);

        let request = CompletionRequest::new(vec![Message::user("test")]);
        mock.complete(request).await.unwrap();

        assert_eq!(mock.request_count(), 1);
        assert_eq!(
            mock.request_history()[0].messages[0].first_text(),
            Some("test")
        );
    }

    #[tokio::test]
    async fn test_mock_llm_tool_use_response() {
        let tool_input = serde_json::json!({"path": "/test/file.txt"});
        let mock =
            MockLLM::new().with_response(MockResponse::tool_use("read_file", tool_input.clone()));

        let request = CompletionRequest::new(vec![Message::user("read the file")]);
        let response = mock.complete(request).await.unwrap();

        assert_eq!(response.stop_reason, StopReason::ToolUse);
        assert!(response.message.has_tool_use());

        let tool_uses = response.message.tool_uses();
        assert_eq!(tool_uses.len(), 1);

        if let ContentBlock::ToolUse { name, input, .. } = tool_uses[0] {
            assert_eq!(name, "read_file");
            assert_eq!(input, &tool_input);
        } else {
            panic!("Expected ToolUse block");
        }
    }

    #[tokio::test]
    async fn test_mock_llm_multi_turn_conversation() {
        let mock = MockLLM::new()
            .with_response(MockResponse::tool_use(
                "bash",
                serde_json::json!({"command": "ls"}),
            ))
            .with_response(MockResponse::text(
                "The directory contains: file1.txt, file2.txt",
            ));

        let request1 = CompletionRequest::new(vec![Message::user("list files")]);
        let response1 = mock.complete(request1).await.unwrap();
        assert!(response1.message.has_tool_use());

        let mut messages = vec![Message::user("list files"), response1.message];

        if let ContentBlock::ToolUse { id, .. } = &messages[1].content[0] {
            messages.push(Message::new(
                Role::Tool,
                vec![ContentBlock::tool_result(id, "file1.txt\nfile2.txt")],
            ));
        }

        let request2 = CompletionRequest::new(messages);
        let response2 = mock.complete(request2).await.unwrap();
        assert_eq!(
            response2.message.first_text(),
            Some("The directory contains: file1.txt, file2.txt")
        );
    }
}
