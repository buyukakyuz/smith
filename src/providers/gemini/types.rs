#![allow(dead_code)]

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiRequest {
    pub contents: Vec<Content>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_instruction: Option<Content>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation_config: Option<GenerationConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Content {
    pub role: String,
    pub parts: Vec<Part>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum Part {
    Text {
        text: String,
    },
    InlineData {
        #[serde(rename = "inlineData")]
        inline_data: InlineData,
    },
    FunctionCall {
        #[serde(rename = "functionCall")]
        function_call: FunctionCall,
        #[serde(rename = "thoughtSignature", skip_serializing_if = "Option::is_none")]
        thought_signature: Option<String>,
    },
    FunctionResponse {
        #[serde(rename = "functionResponse")]
        function_response: FunctionResponse,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InlineData {
    pub mime_type: String,
    pub data: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FunctionCall {
    pub name: String,
    pub args: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FunctionResponse {
    pub name: String,
    pub response: FunctionResponseContent,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FunctionResponseContent {
    pub content: serde_json::Value,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tool {
    pub function_declarations: Vec<FunctionDeclaration>,
}

#[derive(Debug, Serialize)]
pub struct FunctionDeclaration {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiResponse {
    #[serde(default)]
    pub candidates: Vec<Candidate>,
    #[serde(default)]
    pub usage_metadata: Option<UsageMetadata>,
    #[serde(default)]
    pub prompt_feedback: Option<PromptFeedback>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub content: Option<Content>,
    #[serde(default)]
    pub finish_reason: Option<String>,
    #[serde(default)]
    pub safety_ratings: Vec<SafetyRating>,
    #[serde(default)]
    pub index: usize,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SafetyRating {
    pub category: String,
    pub probability: String,
    #[serde(default)]
    pub blocked: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageMetadata {
    #[serde(default)]
    pub prompt_token_count: u32,
    #[serde(default)]
    pub candidates_token_count: u32,
    #[serde(default)]
    pub total_token_count: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptFeedback {
    #[serde(default)]
    pub block_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiError {
    pub error: ErrorDetail,
}

#[derive(Debug, Deserialize)]
pub struct ErrorDetail {
    pub code: u16,
    pub message: String,
    #[serde(default)]
    pub status: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_serialization() {
        let request = ApiRequest {
            contents: vec![Content {
                role: "user".to_string(),
                parts: vec![Part::Text {
                    text: "Hello".to_string(),
                }],
            }],
            system_instruction: None,
            tools: None,
            generation_config: Some(GenerationConfig {
                max_output_tokens: Some(1024),
                temperature: Some(0.7),
                stop_sequences: None,
            }),
        };

        let json = serde_json::to_string(&request).expect("serialize");
        assert!(json.contains("user"));
        assert!(json.contains("Hello"));
        assert!(json.contains("maxOutputTokens"));
    }

    #[test]
    fn test_response_deserialization() {
        let json = r#"{
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [{"text": "Hello!"}]
                },
                "finishReason": "STOP",
                "index": 0
            }],
            "usageMetadata": {
                "promptTokenCount": 10,
                "candidatesTokenCount": 5,
                "totalTokenCount": 15
            }
        }"#;

        let response: ApiResponse = serde_json::from_str(json).expect("deserialize");
        assert_eq!(response.candidates.len(), 1);
        assert_eq!(
            response.candidates[0].finish_reason,
            Some("STOP".to_string())
        );
    }

    #[test]
    fn test_tool_serialization() {
        let tool = Tool {
            function_declarations: vec![FunctionDeclaration {
                name: "get_weather".to_string(),
                description: "Get the weather".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "location": {"type": "string"}
                    }
                }),
            }],
        };

        let json = serde_json::to_string(&tool).expect("serialize");
        assert!(json.contains("functionDeclarations"));
        assert!(json.contains("get_weather"));
    }
}
