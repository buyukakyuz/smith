#![allow(dead_code)]
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    Thinking {
        thinking: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        signature: Option<String>,
    },
    RedactedThinking {
        data: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
    Image {
        source: ImageSource,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ImageSource {
    Base64 { media_type: String, data: String },
    Url { url: String },
}

impl ContentBlock {
    #[must_use]
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }

    #[must_use]
    pub fn tool_use(name: impl Into<String>, input: serde_json::Value) -> Self {
        Self::ToolUse {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            input,
        }
    }

    #[must_use]
    pub fn tool_result(tool_use_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self::ToolResult {
            tool_use_id: tool_use_id.into(),
            content: content.into(),
            is_error: None,
        }
    }

    #[must_use]
    pub fn tool_error(tool_use_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self::ToolResult {
            tool_use_id: tool_use_id.into(),
            content: content.into(),
            is_error: Some(true),
        }
    }

    #[must_use]
    pub fn image_base64(media_type: impl Into<String>, data: impl Into<String>) -> Self {
        Self::Image {
            source: ImageSource::Base64 {
                media_type: media_type.into(),
                data: data.into(),
            },
        }
    }

    #[must_use]
    pub fn image_url(url: impl Into<String>) -> Self {
        Self::Image {
            source: ImageSource::Url { url: url.into() },
        }
    }

    #[must_use]
    pub const fn is_text(&self) -> bool {
        matches!(self, Self::Text { .. })
    }

    #[must_use]
    pub const fn is_thinking(&self) -> bool {
        matches!(self, Self::Thinking { .. })
    }

    #[must_use]
    pub const fn is_redacted_thinking(&self) -> bool {
        matches!(self, Self::RedactedThinking { .. })
    }

    #[must_use]
    pub const fn is_tool_use(&self) -> bool {
        matches!(self, Self::ToolUse { .. })
    }

    #[must_use]
    pub const fn is_tool_result(&self) -> bool {
        matches!(self, Self::ToolResult { .. })
    }

    #[must_use]
    pub fn as_text(&self) -> Option<&str> {
        if let Self::Text { text } = self {
            Some(text)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: Vec<ContentBlock>,
}

impl Message {
    #[must_use]
    pub const fn new(role: Role, content: Vec<ContentBlock>) -> Self {
        Self { role, content }
    }

    #[must_use]
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: vec![ContentBlock::text(text)],
        }
    }

    #[must_use]
    pub fn assistant(text: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: vec![ContentBlock::text(text)],
        }
    }

    #[must_use]
    pub fn system(text: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: vec![ContentBlock::text(text)],
        }
    }

    pub fn add_content(&mut self, block: ContentBlock) {
        self.content.push(block);
    }

    #[must_use]
    pub fn has_tool_use(&self) -> bool {
        self.content.iter().any(ContentBlock::is_tool_use)
    }

    #[must_use]
    pub fn tool_uses(&self) -> Vec<&ContentBlock> {
        self.content.iter().filter(|b| b.is_tool_use()).collect()
    }

    #[must_use]
    pub fn first_text(&self) -> Option<&str> {
        self.content.iter().find_map(ContentBlock::as_text)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

impl ToolDefinition {
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        input_schema: serde_json::Value,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    EndTurn,
    ToolUse,
    MaxTokens,
    StopSequence,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

impl Usage {
    #[must_use]
    pub const fn new(input_tokens: u32, output_tokens: u32) -> Self {
        Self {
            input_tokens,
            output_tokens,
        }
    }

    #[must_use]
    pub const fn total(&self) -> u32 {
        self.input_tokens + self.output_tokens
    }

    pub const fn add(&mut self, other: &Self) {
        self.input_tokens += other.input_tokens;
        self.output_tokens += other.output_tokens;
    }
}

#[derive(Debug, Clone)]
pub struct CompletionRequest {
    pub messages: Vec<Message>,
    pub system_prompt: Option<String>,
    pub tools: Vec<ToolDefinition>,
    pub max_tokens: u32,
    pub temperature: f32,
    pub stop_sequences: Vec<String>,
}

impl CompletionRequest {
    #[must_use]
    pub const fn new(messages: Vec<Message>) -> Self {
        Self {
            messages,
            system_prompt: None,
            tools: Vec::new(),
            max_tokens: 4096,
            temperature: 1.0,
            stop_sequences: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    #[must_use]
    pub fn with_tools(mut self, tools: Vec<ToolDefinition>) -> Self {
        self.tools = tools;
        self
    }

    #[must_use]
    pub const fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    #[must_use]
    pub const fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = temperature;
        self
    }
}
#[derive(Debug, Clone)]
pub struct CompletionResponse {
    pub message: Message,
    pub stop_reason: StopReason,
    pub usage: Usage,
}

impl CompletionResponse {
    #[must_use]
    pub const fn new(message: Message, stop_reason: StopReason, usage: Usage) -> Self {
        Self {
            message,
            stop_reason,
            usage,
        }
    }
}
#[derive(Debug, Clone)]
pub enum StreamEvent {
    ContentBlockStart {
        index: usize,
        content_block: ContentBlock,
    },
    ContentBlockDelta {
        index: usize,
        delta: ContentDelta,
    },
    ContentBlockStop {
        index: usize,
    },
    MessageStart {
        message: Message,
    },
    MessageDelta {
        delta: MessageDelta,
    },
    MessageStop,
}
#[derive(Debug, Clone)]
pub enum ContentDelta {
    TextDelta { text: String },
    ThinkingDelta { thinking: String },
    SignatureDelta { signature: String },
    InputJsonDelta { partial_json: String },
}
#[derive(Debug, Clone)]
pub struct MessageDelta {
    pub stop_reason: Option<StopReason>,
    pub usage: Option<Usage>,
}

pub type StreamResponse =
    futures::stream::BoxStream<'static, crate::core::error::Result<StreamEvent>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_serialization() {
        assert_eq!(serde_json::to_string(&Role::User).unwrap(), "\"user\"");
        assert_eq!(
            serde_json::to_string(&Role::Assistant).unwrap(),
            "\"assistant\""
        );
    }

    #[test]
    fn test_content_block_text() {
        let block = ContentBlock::text("hello");
        assert!(block.is_text());
        assert_eq!(block.as_text(), Some("hello"));
    }

    #[test]
    fn test_content_block_tool_use() {
        let input = serde_json::json!({"arg": "value"});
        let block = ContentBlock::tool_use("test_tool", input.clone());

        assert!(block.is_tool_use());
        if let ContentBlock::ToolUse { name, input: i, .. } = block {
            assert_eq!(name, "test_tool");
            assert_eq!(i, input);
        } else {
            panic!("Expected ToolUse block");
        }
    }

    #[test]
    fn test_message_user() {
        let msg = Message::user("hello");
        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.first_text(), Some("hello"));
    }

    #[test]
    fn test_message_tool_use_detection() {
        let mut msg = Message::assistant("thinking...");
        assert!(!msg.has_tool_use());

        msg.add_content(ContentBlock::tool_use("test", serde_json::json!({})));
        assert!(msg.has_tool_use());
        assert_eq!(msg.tool_uses().len(), 1);
    }

    #[test]
    fn test_usage_total() {
        let usage = Usage::new(100, 50);
        assert_eq!(usage.total(), 150);
    }

    #[test]
    fn test_usage_add() {
        let mut usage1 = Usage::new(100, 50);
        let usage2 = Usage::new(20, 30);
        usage1.add(&usage2);
        assert_eq!(usage1.input_tokens, 120);
        assert_eq!(usage1.output_tokens, 80);
    }

    #[test]
    fn test_message_serialization_roundtrip() {
        let original = Message::user("test message");
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(original, deserialized);
    }
}
