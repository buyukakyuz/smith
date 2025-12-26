#![allow(dead_code)]

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct ApiRequest {
    pub model: String,
    pub input: Vec<InputItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ApiTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_response_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum InputItem {
    #[serde(rename = "message")]
    Message(InputMessage),
    #[serde(rename = "function_call")]
    FunctionCall(InputFunctionCall),
    #[serde(rename = "function_call_output")]
    FunctionCallOutput(FunctionCallOutput),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InputFunctionCall {
    pub call_id: String,
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InputMessage {
    pub role: String,
    pub content: Vec<InputContent>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum InputContent {
    #[serde(rename = "input_text")]
    InputText { text: String },
    #[serde(rename = "input_image")]
    InputImage { image_url: String },
    #[serde(rename = "output_text")]
    OutputText { text: String },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FunctionCallOutput {
    pub call_id: String,
    pub output: String,
}

#[derive(Debug, Serialize)]
pub struct ApiTool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct ApiResponse {
    pub id: String,
    pub status: String,
    pub output: Vec<OutputItem>,
    #[serde(default)]
    pub usage: Option<ApiUsage>,
    #[serde(default)]
    pub error: Option<ApiError>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum OutputItem {
    #[serde(rename = "message")]
    Message(OutputMessage),
    #[serde(rename = "function_call")]
    FunctionCall(FunctionCall),
    #[serde(rename = "reasoning")]
    Reasoning(ReasoningItem),
}

#[derive(Debug, Deserialize, Clone)]
pub struct ReasoningItem {
    pub id: String,
    #[serde(default)]
    pub summary: Vec<ReasoningSummary>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ReasoningSummary {
    #[serde(rename = "type")]
    pub summary_type: String,
    pub text: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct OutputMessage {
    pub id: String,
    pub role: String,
    pub content: Vec<OutputContent>,
    pub status: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum OutputContent {
    #[serde(rename = "output_text")]
    OutputText {
        text: String,
        #[serde(default)]
        annotations: Vec<serde_json::Value>,
    },
}

#[derive(Debug, Deserialize, Clone)]
pub struct FunctionCall {
    pub id: String,
    pub call_id: String,
    pub name: String,
    pub arguments: String,
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct ApiUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub total_tokens: u32,
}
#[derive(Debug, Deserialize)]
pub struct StreamEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    #[serde(flatten)]
    pub data: StreamEventData,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum StreamEventData {
    ResponseCreated {
        response: ApiResponse,
    },
    ResponseDone {
        response: ApiResponse,
    },
    OutputItemAdded {
        output_index: usize,
        item: OutputItem,
    },
    ContentPartAdded {
        output_index: usize,
        content_index: usize,
        part: OutputContent,
    },
    ContentPartDelta {
        output_index: usize,
        content_index: usize,
        delta: ContentDelta,
    },
    FunctionCallArgumentsDelta {
        output_index: usize,
        call_id: String,
        delta: String,
    },
    Empty {},
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum ContentDelta {
    #[serde(rename = "text_delta")]
    TextDelta { text: String },
}
