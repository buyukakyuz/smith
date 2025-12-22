use crate::core::types::{
    CompletionRequest, CompletionResponse, ContentBlock, ContentDelta as CoreContentDelta, Message,
    MessageDelta, Role, StopReason, StreamEvent as CoreStreamEvent, ToolDefinition, Usage,
};
use crate::providers::types::ModelId;

use super::types::{
    ApiRequest, ApiResponse, ApiTool, FunctionCall, FunctionCallOutput, InputContent,
    InputFunctionCall, InputItem, InputMessage, OutputContent, OutputItem, OutputMessage,
};

pub fn to_api_request(model: &ModelId, request: &CompletionRequest) -> ApiRequest {
    let mut input: Vec<InputItem> = Vec::new();

    for msg in &request.messages {
        input.extend(to_input_items(msg));
    }

    let tools: Option<Vec<ApiTool>> = if request.tools.is_empty() {
        None
    } else {
        Some(request.tools.iter().map(to_api_tool).collect())
    };

    ApiRequest {
        model: model.as_str().to_string(),
        input,
        instructions: request.system_prompt.clone(),
        max_output_tokens: Some(request.max_tokens),
        temperature: Some(request.temperature),
        tools,
        stream: None,
        previous_response_id: None,
    }
}

fn to_input_items(message: &Message) -> Vec<InputItem> {
    match message.role {
        Role::User => {
            let content: Vec<InputContent> = message
                .content
                .iter()
                .filter_map(|b| {
                    if let ContentBlock::Text { text } = b {
                        Some(InputContent::InputText { text: text.clone() })
                    } else {
                        None
                    }
                })
                .collect();

            if content.is_empty() {
                return vec![];
            }

            vec![InputItem::Message(InputMessage {
                role: "user".to_string(),
                content,
            })]
        }
        Role::System => {
            vec![]
        }
        Role::Assistant => {
            let mut items = Vec::new();

            let text_content: Vec<InputContent> = message
                .content
                .iter()
                .filter_map(|b| {
                    if let ContentBlock::Text { text } = b {
                        Some(InputContent::OutputText { text: text.clone() })
                    } else {
                        None
                    }
                })
                .collect();

            if !text_content.is_empty() {
                items.push(InputItem::Message(InputMessage {
                    role: "assistant".to_string(),
                    content: text_content,
                }));
            }

            for block in &message.content {
                if let ContentBlock::ToolUse {
                    id, name, input, ..
                } = block
                {
                    items.push(InputItem::FunctionCall(InputFunctionCall {
                        call_id: id.clone(),
                        name: name.clone(),
                        arguments: input.to_string(),
                    }));
                }
            }

            items
        }
        Role::Tool => message
            .content
            .iter()
            .filter_map(|b| {
                if let ContentBlock::ToolResult {
                    tool_use_id,
                    content,
                    ..
                } = b
                {
                    Some(InputItem::FunctionCallOutput(FunctionCallOutput {
                        call_id: tool_use_id.clone(),
                        output: if content.is_empty() {
                            "[No output]".to_string()
                        } else {
                            content.clone()
                        },
                    }))
                } else {
                    None
                }
            })
            .collect(),
    }
}

fn to_api_tool(tool: &ToolDefinition) -> ApiTool {
    ApiTool {
        tool_type: "function".to_string(),
        name: tool.name.clone(),
        description: tool.description.clone(),
        parameters: tool.input_schema.clone(),
    }
}

pub fn from_api_response(response: ApiResponse) -> CompletionResponse {
    let has_function_calls = response
        .output
        .iter()
        .any(|item| matches!(item, OutputItem::FunctionCall(_)));

    let mut content: Vec<ContentBlock> = Vec::new();

    for item in response.output {
        match item {
            OutputItem::Message(msg) => {
                content.extend(from_output_message(&msg));
            }
            OutputItem::FunctionCall(fc) => {
                content.push(from_function_call(&fc));
            }
        }
    }

    let message = Message {
        role: Role::Assistant,
        content,
    };

    let stop_reason = if has_function_calls {
        StopReason::ToolUse
    } else {
        match response.status.as_str() {
            "completed" => StopReason::EndTurn,
            "incomplete" => StopReason::MaxTokens,
            _ => StopReason::EndTurn,
        }
    };

    let usage = response.usage.map_or_else(Usage::default, |u| {
        Usage::new(u.input_tokens, u.output_tokens)
    });

    CompletionResponse::new(message, stop_reason, usage)
}

fn from_output_message(msg: &OutputMessage) -> Vec<ContentBlock> {
    msg.content
        .iter()
        .filter_map(|c| match c {
            OutputContent::OutputText { text, .. } => {
                if text.is_empty() {
                    None
                } else {
                    Some(ContentBlock::Text { text: text.clone() })
                }
            }
        })
        .collect()
}

fn from_function_call(fc: &FunctionCall) -> ContentBlock {
    let input: serde_json::Value =
        serde_json::from_str(&fc.arguments).unwrap_or_else(|_| serde_json::json!({}));

    ContentBlock::ToolUse {
        id: fc.call_id.clone(),
        name: fc.name.clone(),
        input,
        signature: None,
    }
}

#[must_use]
pub fn parse_stream_event(event_type: Option<&str>, data: &str) -> Option<CoreStreamEvent> {
    if data == "[DONE]" {
        return Some(CoreStreamEvent::MessageStop);
    }

    match event_type? {
        "response.output_text.delta" => {
            #[derive(serde::Deserialize)]
            struct TextDelta {
                output_index: Option<usize>,
                delta: String,
            }
            let parsed: TextDelta = serde_json::from_str(data).ok()?;
            Some(CoreStreamEvent::ContentBlockDelta {
                index: parsed.output_index.unwrap_or(0),
                delta: CoreContentDelta::TextDelta { text: parsed.delta },
            })
        }
        "response.function_call_arguments.delta" => {
            #[derive(serde::Deserialize)]
            struct ArgsDelta {
                output_index: usize,
                delta: String,
            }
            let parsed: ArgsDelta = serde_json::from_str(data).ok()?;
            Some(CoreStreamEvent::ContentBlockDelta {
                index: parsed.output_index,
                delta: CoreContentDelta::InputJsonDelta {
                    partial_json: parsed.delta,
                },
            })
        }
        "response.output_item.added" => {
            #[derive(serde::Deserialize)]
            struct ItemAdded {
                output_index: usize,
                item: OutputItem,
            }
            let parsed: ItemAdded = serde_json::from_str(data).ok()?;
            match parsed.item {
                OutputItem::FunctionCall(fc) => Some(CoreStreamEvent::ContentBlockStart {
                    index: parsed.output_index,
                    content_block: ContentBlock::ToolUse {
                        id: fc.call_id,
                        name: fc.name,
                        input: serde_json::json!({}),
                        signature: None,
                    },
                }),
                OutputItem::Message(_) => Some(CoreStreamEvent::ContentBlockStart {
                    index: parsed.output_index,
                    content_block: ContentBlock::Text {
                        text: String::new(),
                    },
                }),
            }
        }
        "response.completed" | "response.done" => Some(CoreStreamEvent::MessageDelta {
            delta: MessageDelta {
                stop_reason: Some(StopReason::EndTurn),
                usage: None,
            },
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_input_items_user() {
        let message = Message::user("Hello");
        let items = to_input_items(&message);

        assert_eq!(items.len(), 1);
        if let InputItem::Message(msg) = &items[0] {
            assert_eq!(msg.role, "user");
        } else {
            panic!("Expected Message");
        }
    }

    #[test]
    fn test_to_api_tool() {
        let tool = ToolDefinition {
            name: "read_file".to_string(),
            description: "Read a file".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"}
                }
            }),
        };

        let api_tool = to_api_tool(&tool);
        assert_eq!(api_tool.tool_type, "function");
        assert_eq!(api_tool.name, "read_file");
        assert_eq!(api_tool.description, "Read a file");
    }
}
