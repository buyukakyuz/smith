#![allow(dead_code)]

use serde::{Deserialize, Serialize};
#[derive(Debug, Serialize)]
pub struct ApiRequest {
    pub model: String,
    pub messages: Vec<ApiMessage>,
    pub max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ApiToolDefinition>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiMessage {
    pub role: String,
    pub content: Vec<ApiContentBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ApiContentBlock {
    Text {
        text: String,
    },
    Thinking {
        thinking: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        signature: Option<String>,
    },
    #[serde(rename = "redacted_thinking")]
    RedactedThinking {
        data: String,
    },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
    Image {
        source: ApiImageSource,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ApiImageSource {
    Base64 { media_type: String, data: String },
    Url { url: String },
}

#[derive(Debug, Serialize)]
pub struct ApiToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct ApiResponse {
    pub content: Vec<ApiContentBlock>,
    pub stop_reason: Option<String>,
    pub usage: ApiUsage,
}

#[derive(Debug, Deserialize)]
pub struct ApiUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

#[derive(Debug, Deserialize)]
pub struct SseEventData {
    #[serde(rename = "type")]
    pub event_type: String,
    #[serde(default)]
    pub index: Option<usize>,
    #[serde(default)]
    pub content_block: Option<ApiContentBlock>,
    #[serde(default)]
    pub delta: Option<SseDelta>,
    #[serde(default)]
    pub message: Option<SseMessage>,
    #[serde(default)]
    pub usage: Option<ApiUsage>,
}

#[derive(Debug, Deserialize)]
pub struct SseDelta {
    #[serde(rename = "type", default)]
    pub delta_type: String,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub thinking: Option<String>,
    #[serde(default)]
    pub signature: Option<String>,
    #[serde(default)]
    pub partial_json: Option<String>,
    #[serde(default)]
    pub stop_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SseMessage {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_use_serialization() {
        let block = ApiContentBlock::ToolUse {
            id: "test-id".to_string(),
            name: "read_file".to_string(),
            input: serde_json::json!({"path": "/tmp/test.txt"}),
        };

        let json = serde_json::to_string(&block).expect("serialize");
        assert!(json.contains("tool_use"));
        assert!(json.contains("read_file"));
    }

    #[test]
    fn test_tool_result_serialization() {
        let block = ApiContentBlock::ToolResult {
            tool_use_id: "test-id".to_string(),
            content: "file contents".to_string(),
            is_error: None,
        };

        let json = serde_json::to_string(&block).expect("serialize");
        assert!(json.contains("tool_result"));
        assert!(json.contains("file contents"));
    }

    #[test]
    fn test_api_request_serialization() {
        let request = ApiRequest {
            model: "claude-sonnet-4".to_string(),
            messages: vec![ApiMessage {
                role: "user".to_string(),
                content: vec![ApiContentBlock::Text {
                    text: "Hello".to_string(),
                }],
            }],
            max_tokens: 4096,
            system: Some("You are helpful".to_string()),
            temperature: Some(0.7),
            tools: None,
            stream: None,
        };

        let json = serde_json::to_string(&request).expect("serialize");
        assert!(json.contains("claude-sonnet-4"));
        assert!(json.contains("Hello"));
        assert!(json.contains("You are helpful"));
    }

    #[test]
    fn test_api_response_deserialization() {
        let json = r#"{
            "content": [{"type": "text", "text": "Hello!"}],
            "stop_reason": "end_turn",
            "usage": {"input_tokens": 10, "output_tokens": 5}
        }"#;

        let response: ApiResponse = serde_json::from_str(json).expect("deserialize");
        assert_eq!(response.stop_reason, Some("end_turn".to_string()));
        assert_eq!(response.usage.input_tokens, 10);
        assert_eq!(response.usage.output_tokens, 5);
    }
}
