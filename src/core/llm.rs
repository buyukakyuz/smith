#![allow(dead_code)]
use async_trait::async_trait;

use super::error::Result;
use super::types::{CompletionRequest, CompletionResponse, StreamResponse};

#[async_trait]
pub trait LLM: Send + Sync {
    fn name(&self) -> &str;
    fn model(&self) -> &str;
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse>;
    async fn stream(&self, request: CompletionRequest) -> Result<StreamResponse>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{Message, StopReason, Usage};

    struct TestLLM;

    #[async_trait]
    impl LLM for TestLLM {
        fn name(&self) -> &'static str {
            "test"
        }

        fn model(&self) -> &'static str {
            "test-model"
        }

        async fn complete(&self, _request: CompletionRequest) -> Result<CompletionResponse> {
            Ok(CompletionResponse::new(
                Message::assistant("test response"),
                StopReason::EndTurn,
                Usage::new(10, 5),
            ))
        }

        async fn stream(&self, _request: CompletionRequest) -> Result<StreamResponse> {
            use futures::stream;
            Ok(Box::pin(stream::empty()))
        }
    }

    #[tokio::test]
    async fn test_llm_trait_object_safe() {
        let llm: Box<dyn LLM> = Box::new(TestLLM);
        assert_eq!(llm.name(), "test");
        assert_eq!(llm.model(), "test-model");

        let request = CompletionRequest::new(vec![Message::user("hello")]);
        let response = llm.complete(request).await.unwrap();
        assert_eq!(response.message.first_text(), Some("test response"));
    }
}
