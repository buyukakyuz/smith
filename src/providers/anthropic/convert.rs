use crate::core::types::{
    CompletionRequest, CompletionResponse, ContentBlock, ContentDelta, ImageSource, Message,
    MessageDelta, Role, StopReason, StreamEvent, ToolDefinition, Usage,
};
use crate::providers::types::ModelId;

use super::types::{
    ApiContentBlock, ApiImageSource, ApiMessage, ApiRequest, ApiResponse, ApiToolDefinition,
    ApiUsage, SseDelta, SseEventData,
};

pub fn to_api_request(model: &ModelId, request: &CompletionRequest) -> ApiRequest {
    let messages: Vec<ApiMessage> = request
        .messages
        .iter()
        .map(to_api_message)
        .filter(|m| !m.content.is_empty())
        .collect();

    let tools: Option<Vec<ApiToolDefinition>> = if request.tools.is_empty() {
        None
    } else {
        Some(request.tools.iter().map(to_api_tool).collect())
    };

    ApiRequest {
        model: model.as_str().to_string(),
        messages,
        max_tokens: request.max_tokens,
        system: request.system_prompt.clone(),
        temperature: Some(request.temperature),
        tools,
        stream: None,
    }
}

fn to_api_message(message: &Message) -> ApiMessage {
    let role = match message.role {
        Role::User | Role::Tool | Role::System => "user".to_string(),
        Role::Assistant => "assistant".to_string(),
    };

    let content: Vec<ApiContentBlock> = message
        .content
        .iter()
        .filter(|block| !matches!(block, ContentBlock::Text { text } if text.is_empty()))
        .map(to_api_content_block)
        .collect();

    ApiMessage { role, content }
}

fn to_api_content_block(block: &ContentBlock) -> ApiContentBlock {
    match block {
        ContentBlock::Text { text } => ApiContentBlock::Text { text: text.clone() },
        ContentBlock::Thinking {
            thinking,
            signature,
        } => ApiContentBlock::Thinking {
            thinking: thinking.clone(),
            signature: signature.clone(),
        },
        ContentBlock::RedactedThinking { data } => {
            ApiContentBlock::RedactedThinking { data: data.clone() }
        }
        ContentBlock::ToolUse { id, name, input } => ApiContentBlock::ToolUse {
            id: id.clone(),
            name: name.clone(),
            input: input.clone(),
        },
        ContentBlock::ToolResult {
            tool_use_id,
            content,
            is_error,
        } => ApiContentBlock::ToolResult {
            tool_use_id: tool_use_id.clone(),
            content: content.clone(),
            is_error: *is_error,
        },
        ContentBlock::Image { source } => ApiContentBlock::Image {
            source: match source {
                ImageSource::Base64 { media_type, data } => ApiImageSource::Base64 {
                    media_type: media_type.clone(),
                    data: data.clone(),
                },
                ImageSource::Url { url } => ApiImageSource::Url { url: url.clone() },
            },
        },
    }
}

fn to_api_tool(tool: &ToolDefinition) -> ApiToolDefinition {
    ApiToolDefinition {
        name: tool.name.clone(),
        description: tool.description.clone(),
        input_schema: tool.input_schema.clone(),
    }
}

pub fn from_api_response(response: ApiResponse) -> CompletionResponse {
    let content: Vec<ContentBlock> = response
        .content
        .into_iter()
        .map(from_api_content_block)
        .collect();

    let message = Message {
        role: Role::Assistant,
        content,
    };

    let stop_reason = match response.stop_reason.as_deref() {
        Some("tool_use") => StopReason::ToolUse,
        Some("max_tokens") => StopReason::MaxTokens,
        Some("stop_sequence") => StopReason::StopSequence,
        _ => StopReason::EndTurn,
    };

    let usage = from_api_usage(&response.usage);

    CompletionResponse::new(message, stop_reason, usage)
}

fn from_api_content_block(block: ApiContentBlock) -> ContentBlock {
    match block {
        ApiContentBlock::Text { text } => ContentBlock::Text { text },
        ApiContentBlock::Thinking {
            thinking,
            signature,
        } => ContentBlock::Thinking {
            thinking,
            signature,
        },
        ApiContentBlock::RedactedThinking { data } => ContentBlock::RedactedThinking { data },
        ApiContentBlock::ToolUse { id, name, input } => ContentBlock::ToolUse { id, name, input },
        ApiContentBlock::ToolResult {
            tool_use_id,
            content,
            is_error,
        } => ContentBlock::ToolResult {
            tool_use_id,
            content,
            is_error,
        },
        ApiContentBlock::Image { .. } => ContentBlock::Text {
            text: "[Image content]".to_string(),
        },
    }
}

const fn from_api_usage(usage: &ApiUsage) -> Usage {
    Usage::new(usage.input_tokens, usage.output_tokens)
}

pub fn parse_stream_event(data: &str) -> Option<StreamEvent> {
    let event: SseEventData = serde_json::from_str(data).ok()?;

    match event.event_type.as_str() {
        "message_start" => event.message.map(|_| StreamEvent::MessageStart {
            message: Message {
                role: Role::Assistant,
                content: vec![],
            },
        }),
        "content_block_start" => {
            let index = event.index.unwrap_or(0);
            let block = event.content_block.map(from_api_content_block)?;
            Some(StreamEvent::ContentBlockStart {
                index,
                content_block: block,
            })
        }
        "content_block_delta" => {
            let index = event.index.unwrap_or(0);
            let delta = event.delta?;
            let content_delta = parse_content_delta(&delta)?;
            Some(StreamEvent::ContentBlockDelta {
                index,
                delta: content_delta,
            })
        }
        "content_block_stop" => {
            let index = event.index.unwrap_or(0);
            Some(StreamEvent::ContentBlockStop { index })
        }
        "message_delta" => {
            let delta = event.delta?;
            let stop_reason = delta.stop_reason.map(|s| match s.as_str() {
                "tool_use" => StopReason::ToolUse,
                "max_tokens" => StopReason::MaxTokens,
                "stop_sequence" => StopReason::StopSequence,
                _ => StopReason::EndTurn,
            });
            Some(StreamEvent::MessageDelta {
                delta: MessageDelta {
                    stop_reason,
                    usage: event.usage.as_ref().map(from_api_usage),
                },
            })
        }
        "message_stop" => Some(StreamEvent::MessageStop),
        _ => None,
    }
}

fn parse_content_delta(delta: &SseDelta) -> Option<ContentDelta> {
    match delta.delta_type.as_str() {
        "text_delta" => Some(ContentDelta::TextDelta {
            text: delta.text.clone().unwrap_or_default(),
        }),
        "thinking_delta" => Some(ContentDelta::ThinkingDelta {
            thinking: delta.thinking.clone().unwrap_or_default(),
        }),
        "signature_delta" => Some(ContentDelta::SignatureDelta {
            signature: delta.signature.clone().unwrap_or_default(),
        }),
        "input_json_delta" => Some(ContentDelta::InputJsonDelta {
            partial_json: delta.partial_json.clone().unwrap_or_default(),
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_api_message_user() {
        let message = Message::user("Hello");
        let api_message = to_api_message(&message);

        assert_eq!(api_message.role, "user");
        assert_eq!(api_message.content.len(), 1);
    }

    #[test]
    fn test_to_api_message_assistant() {
        let message = Message::assistant("Hi there");
        let api_message = to_api_message(&message);

        assert_eq!(api_message.role, "assistant");
    }

    #[test]
    fn test_to_api_request() {
        let model = ModelId::new("claude-sonnet-4");
        let request =
            CompletionRequest::new(vec![Message::user("Hello")]).with_system_prompt("Be helpful");

        let api_request = to_api_request(&model, &request);

        assert_eq!(api_request.model, "claude-sonnet-4");
        assert_eq!(api_request.system, Some("Be helpful".to_string()));
        assert_eq!(api_request.messages.len(), 1);
    }

    #[test]
    fn test_from_api_response() {
        let api_response = ApiResponse {
            content: vec![ApiContentBlock::Text {
                text: "Hello!".to_string(),
            }],
            stop_reason: Some("end_turn".to_string()),
            usage: ApiUsage {
                input_tokens: 10,
                output_tokens: 5,
            },
        };

        let response = from_api_response(api_response);

        assert_eq!(response.message.first_text(), Some("Hello!"));
        assert_eq!(response.stop_reason, StopReason::EndTurn);
        assert_eq!(response.usage.input_tokens, 10);
        assert_eq!(response.usage.output_tokens, 5);
    }

    #[test]
    fn test_parse_stream_event_text_delta() {
        let json = r#"{"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": "Hello"}}"#;
        let event = parse_stream_event(json);

        assert!(event.is_some());
        if let Some(StreamEvent::ContentBlockDelta { index, delta }) = event {
            assert_eq!(index, 0);
            if let ContentDelta::TextDelta { text } = delta {
                assert_eq!(text, "Hello");
            } else {
                panic!("Expected TextDelta");
            }
        } else {
            panic!("Expected ContentBlockDelta");
        }
    }

    #[test]
    fn test_parse_stream_event_message_stop() {
        let json = r#"{"type": "message_stop"}"#;
        let event = parse_stream_event(json);

        assert!(matches!(event, Some(StreamEvent::MessageStop)));
    }

    #[test]
    fn test_to_api_message_filters_empty_text_blocks() {
        let message = Message::new(
            Role::Assistant,
            vec![
                ContentBlock::Text {
                    text: String::new(),
                },
                ContentBlock::Text {
                    text: "Hello".to_string(),
                },
            ],
        );
        let api_message = to_api_message(&message);

        assert_eq!(api_message.content.len(), 1);
        assert!(matches!(
            &api_message.content[0],
            ApiContentBlock::Text { text } if text == "Hello"
        ));
    }
}
