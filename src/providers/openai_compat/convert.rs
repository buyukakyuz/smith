use std::collections::HashMap;

use crate::core::types::{
    CompletionRequest, CompletionResponse, ContentBlock, ContentDelta, ImageSource, Message,
    MessageDelta as CoreMessageDelta, Role, StopReason, StreamEvent as CoreStreamEvent,
    ToolDefinition, Usage,
};
use crate::providers::types::ModelId;

use super::config::OpenAICompatConfig;
use super::types::{
    ChatCompletionRequest, ChatCompletionResponse, ChatMessage, ContentPart, FunctionCall,
    FunctionDefinition, ImageUrl, MessageContent, Tool, ToolCall,
};

pub fn to_api_request(
    config: &OpenAICompatConfig,
    request: &CompletionRequest,
) -> ChatCompletionRequest {
    let mut messages: Vec<ChatMessage> = Vec::new();

    if let Some(system_prompt) = &request.system_prompt {
        messages.push(ChatMessage {
            role: "system".to_string(),
            content: Some(MessageContent::Text(system_prompt.clone())),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        });
    }

    for msg in &request.messages {
        messages.extend(to_chat_messages(msg));
    }

    let tools = if config.capabilities.tools && !request.tools.is_empty() {
        Some(request.tools.iter().map(to_api_tool).collect())
    } else {
        None
    };

    let stop = if !request.stop_sequences.is_empty() {
        Some(request.stop_sequences.clone())
    } else {
        None
    };

    ChatCompletionRequest {
        model: config.resolve_model(&ModelId::default().as_str()),
        messages,
        temperature: Some(request.temperature),
        max_tokens: Some(request.max_tokens),
        stream: None,
        tools,
        tool_choice: None,
        stop,
        top_p: None,
        frequency_penalty: None,
        presence_penalty: None,
        user: None,
    }
}

fn to_chat_messages(message: &Message) -> Vec<ChatMessage> {
    match message.role {
        Role::System => {
            vec![]
        }
        Role::User => {
            vec![to_user_message(message)]
        }
        Role::Assistant => to_assistant_messages(message),
        Role::Tool => to_tool_messages(message),
    }
}

fn to_user_message(message: &Message) -> ChatMessage {
    let has_images = message
        .content
        .iter()
        .any(|block| matches!(block, ContentBlock::Image { .. }));

    let content = if has_images {
        let parts: Vec<ContentPart> = message
            .content
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Text { text } => Some(ContentPart::Text { text: text.clone() }),
                ContentBlock::Image { source } => {
                    let url = match source {
                        ImageSource::Base64 { media_type, data } => {
                            format!("data:{};base64,{}", media_type, data)
                        }
                        ImageSource::Url { url } => url.clone(),
                    };
                    Some(ContentPart::ImageUrl {
                        image_url: ImageUrl { url, detail: None },
                    })
                }
                _ => None,
            })
            .collect();

        if parts.is_empty() {
            None
        } else {
            Some(MessageContent::Parts(parts))
        }
    } else {
        let text: String = message
            .content
            .iter()
            .filter_map(|block| {
                if let ContentBlock::Text { text } = block {
                    Some(text.as_str())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        if text.is_empty() {
            None
        } else {
            Some(MessageContent::Text(text))
        }
    };

    ChatMessage {
        role: "user".to_string(),
        content,
        name: None,
        tool_calls: None,
        tool_call_id: None,
    }
}

fn to_assistant_messages(message: &Message) -> Vec<ChatMessage> {
    let text_content: String = message
        .content
        .iter()
        .filter_map(|block| {
            if let ContentBlock::Text { text } = block {
                Some(text.as_str())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    let tool_calls: Vec<ToolCall> = message
        .content
        .iter()
        .filter_map(|block| {
            if let ContentBlock::ToolUse {
                id, name, input, ..
            } = block
            {
                Some(ToolCall {
                    id: id.clone(),
                    tool_type: "function".to_string(),
                    function: FunctionCall {
                        name: name.clone(),
                        arguments: input.to_string(),
                    },
                })
            } else {
                None
            }
        })
        .collect();

    let content = if text_content.is_empty() {
        None
    } else {
        Some(MessageContent::Text(text_content))
    };

    let tool_calls_opt = if tool_calls.is_empty() {
        None
    } else {
        Some(tool_calls)
    };

    vec![ChatMessage {
        role: "assistant".to_string(),
        content,
        name: None,
        tool_calls: tool_calls_opt,
        tool_call_id: None,
    }]
}

fn to_tool_messages(message: &Message) -> Vec<ChatMessage> {
    message
        .content
        .iter()
        .filter_map(|block| {
            if let ContentBlock::ToolResult {
                tool_use_id,
                content,
                ..
            } = block
            {
                Some(ChatMessage {
                    role: "tool".to_string(),
                    content: Some(MessageContent::Text(content.clone())),
                    name: None,
                    tool_calls: None,
                    tool_call_id: Some(tool_use_id.clone()),
                })
            } else {
                None
            }
        })
        .collect()
}

fn to_api_tool(tool: &ToolDefinition) -> Tool {
    Tool {
        tool_type: "function".to_string(),
        function: FunctionDefinition {
            name: tool.name.clone(),
            description: Some(tool.description.clone()),
            parameters: tool.input_schema.clone(),
        },
    }
}

pub fn from_api_response(response: ChatCompletionResponse) -> CompletionResponse {
    let choice = response
        .choices
        .first()
        .expect("ChatCompletionResponse should have at least one choice");

    let mut content: Vec<ContentBlock> = Vec::new();

    if let Some(text) = &choice.message.content {
        if !text.is_empty() {
            content.push(ContentBlock::Text { text: text.clone() });
        }
    }

    if let Some(tool_calls) = &choice.message.tool_calls {
        for tool_call in tool_calls {
            let input: serde_json::Value = serde_json::from_str(&tool_call.function.arguments)
                .unwrap_or_else(|_| serde_json::json!({}));

            content.push(ContentBlock::ToolUse {
                id: tool_call.id.clone(),
                name: tool_call.function.name.clone(),
                input,
                signature: None,
            });
        }
    }

    let message = Message {
        role: Role::Assistant,
        content,
    };

    let stop_reason = match choice.finish_reason.as_deref() {
        Some("stop") => StopReason::EndTurn,
        Some("length") => StopReason::MaxTokens,
        Some("tool_calls") | Some("function_call") => StopReason::ToolUse,
        Some("stop_sequence") | Some("content_filter") => StopReason::StopSequence,
        _ => StopReason::EndTurn,
    };

    let usage = response.usage.map_or_else(Usage::default, |u| {
        Usage::new(u.prompt_tokens, u.completion_tokens)
    });

    CompletionResponse::new(message, stop_reason, usage)
}

#[derive(Debug, Default)]
pub struct StreamState {
    tool_calls: HashMap<usize, PartialToolCall>,
    message_started: bool,
    current_index: usize,
}

#[derive(Debug, Clone)]
struct PartialToolCall {
    id: Option<String>,
    name: Option<String>,
    arguments: String,
}

impl StreamState {
    pub fn new() -> Self {
        Self::default()
    }

    fn get_or_create_tool_call(&mut self, index: usize) -> &mut PartialToolCall {
        self.tool_calls
            .entry(index)
            .or_insert_with(|| PartialToolCall {
                id: None,
                name: None,
                arguments: String::new(),
            })
    }
}

pub fn parse_stream_event(data: &str, state: &mut StreamState) -> Option<CoreStreamEvent> {
    if data.trim() == "[DONE]" {
        return Some(CoreStreamEvent::MessageStop);
    }

    let chunk: super::types::ChatCompletionChunk = serde_json::from_str(data).ok()?;

    let choice = chunk.choices.first()?;

    if !state.message_started {
        state.message_started = true;
        return Some(CoreStreamEvent::MessageStart {
            message: Message {
                role: Role::Assistant,
                content: vec![],
            },
            usage: None,
        });
    }

    if let Some(content) = &choice.delta.content {
        return Some(CoreStreamEvent::ContentBlockDelta {
            index: choice.index as usize,
            delta: ContentDelta::TextDelta {
                text: content.clone(),
            },
        });
    }

    if let Some(tool_call_deltas) = &choice.delta.tool_calls {
        for tool_call_delta in tool_call_deltas {
            let index = tool_call_delta.index as usize;
            let partial = state.get_or_create_tool_call(index);

            if let Some(id) = &tool_call_delta.id {
                partial.id = Some(id.clone());
            }

            if let Some(function) = &tool_call_delta.function {
                if let Some(name) = &function.name {
                    partial.name = Some(name.clone());

                    if let (Some(id), Some(name)) = (&partial.id, &partial.name) {
                        return Some(CoreStreamEvent::ContentBlockStart {
                            index,
                            content_block: ContentBlock::ToolUse {
                                id: id.clone(),
                                name: name.clone(),
                                input: serde_json::json!({}),
                                signature: None,
                            },
                        });
                    }
                }

                if let Some(args) = &function.arguments {
                    partial.arguments.push_str(args);
                    return Some(CoreStreamEvent::ContentBlockDelta {
                        index,
                        delta: ContentDelta::InputJsonDelta {
                            partial_json: args.clone(),
                        },
                    });
                }
            }
        }
    }

    if let Some(finish_reason) = &choice.finish_reason {
        let stop_reason = match finish_reason.as_str() {
            "stop" => StopReason::EndTurn,
            "length" => StopReason::MaxTokens,
            "tool_calls" | "function_call" => StopReason::ToolUse,
            "stop_sequence" | "content_filter" => StopReason::StopSequence,
            _ => StopReason::EndTurn,
        };

        let usage = chunk
            .usage
            .map(|u| Usage::new(u.prompt_tokens, u.completion_tokens));

        return Some(CoreStreamEvent::MessageDelta {
            delta: CoreMessageDelta {
                stop_reason: Some(stop_reason),
                usage,
            },
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::openai_compat::config::ProviderCapabilities;
    use crate::providers::types::ApiKey;

    fn test_config() -> OpenAICompatConfig {
        OpenAICompatConfig::custom("test", "https://api.example.com")
            .with_bearer_auth(ApiKey::new("test-key"))
            .with_capabilities(ProviderCapabilities {
                vision: true,
                tools: true,
                streaming: true,
                parallel_tool_calls: true,
                json_mode: true,
            })
    }

    #[test]
    fn test_to_user_message_simple_text() {
        let message = Message::user("Hello, world!");
        let chat_message = to_user_message(&message);

        assert_eq!(chat_message.role, "user");
        assert!(matches!(
            chat_message.content,
            Some(MessageContent::Text(ref text)) if text == "Hello, world!"
        ));
    }

    #[test]
    fn test_to_user_message_with_image() {
        let mut message = Message::user("Look at this:");
        message.add_content(ContentBlock::image_url("https://example.com/image.png"));

        let chat_message = to_user_message(&message);

        assert_eq!(chat_message.role, "user");
        if let Some(MessageContent::Parts(parts)) = chat_message.content {
            assert_eq!(parts.len(), 2);
            assert!(matches!(parts[0], ContentPart::Text { .. }));
            assert!(matches!(parts[1], ContentPart::ImageUrl { .. }));
        } else {
            panic!("Expected Parts content");
        }
    }

    #[test]
    fn test_to_assistant_messages_with_text() {
        let message = Message::assistant("Here's my response");
        let chat_messages = to_assistant_messages(&message);

        assert_eq!(chat_messages.len(), 1);
        assert_eq!(chat_messages[0].role, "assistant");
        assert!(matches!(
            chat_messages[0].content,
            Some(MessageContent::Text(ref text)) if text == "Here's my response"
        ));
    }

    #[test]
    fn test_to_assistant_messages_with_tool_call() {
        let mut message = Message::assistant("Let me check that");
        message.add_content(ContentBlock::tool_use(
            "read_file",
            serde_json::json!({"path": "/tmp/test.txt"}),
        ));

        let chat_messages = to_assistant_messages(&message);

        assert_eq!(chat_messages.len(), 1);
        assert!(chat_messages[0].content.is_some());
        assert!(chat_messages[0].tool_calls.is_some());

        let tool_calls = chat_messages[0].tool_calls.as_ref().unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].function.name, "read_file");
    }

    #[test]
    fn test_to_tool_messages() {
        let mut message = Message::new(Role::Tool, vec![]);
        message.add_content(ContentBlock::tool_result("call_123", "File contents here"));

        let chat_messages = to_tool_messages(&message);

        assert_eq!(chat_messages.len(), 1);
        assert_eq!(chat_messages[0].role, "tool");
        assert_eq!(chat_messages[0].tool_call_id, Some("call_123".to_string()));
        assert!(matches!(
            chat_messages[0].content,
            Some(MessageContent::Text(ref text)) if text == "File contents here"
        ));
    }

    #[test]
    fn test_to_api_request() {
        let config = test_config();
        let request = CompletionRequest::new(vec![Message::user("Hello")])
            .with_system_prompt("Be helpful")
            .with_max_tokens(1000)
            .with_temperature(0.7);

        let api_request = to_api_request(&config, &request);

        assert_eq!(api_request.messages.len(), 2);
        assert_eq!(api_request.messages[0].role, "system");
        assert_eq!(api_request.messages[1].role, "user");
        assert_eq!(api_request.max_tokens, Some(1000));
        assert_eq!(api_request.temperature, Some(0.7));
    }

    #[test]
    fn test_to_api_request_with_tools() {
        let config = test_config();
        let tool = ToolDefinition::new(
            "read_file",
            "Read a file",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"}
                }
            }),
        );

        let request = CompletionRequest::new(vec![Message::user("Hello")]).with_tools(vec![tool]);

        let api_request = to_api_request(&config, &request);

        assert!(api_request.tools.is_some());
        let tools = api_request.tools.unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].function.name, "read_file");
        assert_eq!(tools[0].tool_type, "function");
    }

    #[test]
    fn test_from_api_response_text_only() {
        let response = ChatCompletionResponse {
            id: "test-id".to_string(),
            object: "chat.completion".to_string(),
            created: 1234567890,
            model: "test-model".to_string(),
            choices: vec![super::super::types::Choice {
                index: 0,
                message: super::super::types::ResponseMessage {
                    role: "assistant".to_string(),
                    content: Some("Hello there!".to_string()),
                    tool_calls: None,
                    refusal: None,
                },
                finish_reason: Some("stop".to_string()),
                logprobs: None,
            }],
            usage: Some(super::super::types::Usage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
            }),
            system_fingerprint: None,
        };

        let completion = from_api_response(response);

        assert_eq!(completion.message.role, Role::Assistant);
        assert_eq!(completion.message.first_text(), Some("Hello there!"));
        assert_eq!(completion.stop_reason, StopReason::EndTurn);
        assert_eq!(completion.usage.input_tokens, 10);
        assert_eq!(completion.usage.output_tokens, 5);
    }

    #[test]
    fn test_from_api_response_with_tool_calls() {
        let response = ChatCompletionResponse {
            id: "test-id".to_string(),
            object: "chat.completion".to_string(),
            created: 1234567890,
            model: "test-model".to_string(),
            choices: vec![super::super::types::Choice {
                index: 0,
                message: super::super::types::ResponseMessage {
                    role: "assistant".to_string(),
                    content: Some("Let me read that file".to_string()),
                    tool_calls: Some(vec![ToolCall {
                        id: "call_123".to_string(),
                        tool_type: "function".to_string(),
                        function: FunctionCall {
                            name: "read_file".to_string(),
                            arguments: r#"{"path":"/tmp/test.txt"}"#.to_string(),
                        },
                    }]),
                    refusal: None,
                },
                finish_reason: Some("tool_calls".to_string()),
                logprobs: None,
            }],
            usage: Some(super::super::types::Usage {
                prompt_tokens: 20,
                completion_tokens: 10,
                total_tokens: 30,
            }),
            system_fingerprint: None,
        };

        let completion = from_api_response(response);

        assert_eq!(completion.message.role, Role::Assistant);
        assert_eq!(completion.message.content.len(), 2);
        assert!(completion.message.has_tool_use());
        assert_eq!(completion.stop_reason, StopReason::ToolUse);
    }

    #[test]
    fn test_parse_stream_event_done() {
        let mut state = StreamState::new();
        let event = parse_stream_event("[DONE]", &mut state);

        assert!(matches!(event, Some(CoreStreamEvent::MessageStop)));
    }

    #[test]
    fn test_parse_stream_event_text_delta() {
        let mut state = StreamState::new();

        let json1 = r#"{"id":"test","object":"chat.completion.chunk","created":123,"model":"test","choices":[{"index":0,"delta":{"role":"assistant","content":""},"finish_reason":null}]}"#;
        let event1 = parse_stream_event(json1, &mut state);
        assert!(matches!(event1, Some(CoreStreamEvent::MessageStart { .. })));

        let json2 = r#"{"id":"test","object":"chat.completion.chunk","created":123,"model":"test","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}"#;
        let event2 = parse_stream_event(json2, &mut state);

        if let Some(CoreStreamEvent::ContentBlockDelta { index, delta }) = event2 {
            assert_eq!(index, 0);
            assert!(matches!(delta, ContentDelta::TextDelta { text } if text == "Hello"));
        } else {
            panic!("Expected ContentBlockDelta with TextDelta");
        }
    }

    #[test]
    fn test_parse_stream_event_finish_reason() {
        let mut state = StreamState::new();
        state.message_started = true;

        let json = r#"{"id":"test","object":"chat.completion.chunk","created":123,"model":"test","choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15}}"#;
        let event = parse_stream_event(json, &mut state);

        if let Some(CoreStreamEvent::MessageDelta { delta }) = event {
            assert_eq!(delta.stop_reason, Some(StopReason::EndTurn));
            assert!(delta.usage.is_some());
            assert_eq!(delta.usage.unwrap().input_tokens, 10);
        } else {
            panic!("Expected MessageDelta");
        }
    }

    #[test]
    fn test_parse_stream_event_tool_call() {
        let mut state = StreamState::new();
        state.message_started = true;

        let json1 = r#"{"id":"test","object":"chat.completion.chunk","created":123,"model":"test","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_123","type":"function","function":{"name":"read_file","arguments":""}}]},"finish_reason":null}]}"#;
        let event1 = parse_stream_event(json1, &mut state);

        if let Some(CoreStreamEvent::ContentBlockStart {
            index,
            content_block,
        }) = event1
        {
            assert_eq!(index, 0);
            assert!(matches!(
                content_block,
                ContentBlock::ToolUse { name, .. } if name == "read_file"
            ));
        } else {
            panic!("Expected ContentBlockStart");
        }

        let json2 = r#"{"id":"test","object":"chat.completion.chunk","created":123,"model":"test","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"path\":\"/tmp/test.txt\"}"}}]},"finish_reason":null}]}"#;
        let event2 = parse_stream_event(json2, &mut state);

        if let Some(CoreStreamEvent::ContentBlockDelta { index, delta }) = event2 {
            assert_eq!(index, 0);
            assert!(matches!(
                delta,
                ContentDelta::InputJsonDelta { partial_json } if partial_json.contains("path")
            ));
        } else {
            panic!("Expected ContentBlockDelta with InputJsonDelta");
        }
    }
}
