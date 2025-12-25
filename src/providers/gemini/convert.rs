use uuid::Uuid;

use crate::core::types::{
    CompletionRequest, CompletionResponse, ContentBlock, ContentDelta as CoreContentDelta,
    ImageSource, Message, MessageDelta, Role, StopReason, StreamEvent as CoreStreamEvent,
    ToolDefinition, Usage,
};

use super::types::{
    ApiRequest, ApiResponse, Content, FunctionCall, FunctionDeclaration, FunctionResponse,
    FunctionResponseContent, GenerationConfig, InlineData, Part, Tool,
};

pub fn to_api_request(request: &CompletionRequest) -> ApiRequest {
    let contents: Vec<Content> = request.messages.iter().filter_map(to_content).collect();

    let system_instruction = request.system_prompt.as_ref().map(|prompt| Content {
        role: "user".to_string(),
        parts: vec![Part::Text {
            text: prompt.clone(),
        }],
    });

    let tools: Option<Vec<Tool>> = if request.tools.is_empty() {
        None
    } else {
        Some(vec![Tool {
            function_declarations: request.tools.iter().map(to_function_declaration).collect(),
        }])
    };

    let generation_config = Some(GenerationConfig {
        max_output_tokens: Some(request.max_tokens),
        temperature: Some(request.temperature),
        stop_sequences: if request.stop_sequences.is_empty() {
            None
        } else {
            Some(request.stop_sequences.clone())
        },
    });

    ApiRequest {
        contents,
        system_instruction,
        tools,
        generation_config,
    }
}

fn to_content(message: &Message) -> Option<Content> {
    let role = match message.role {
        Role::User => "user",
        Role::Assistant => "model",
        Role::Tool => return Some(to_tool_response_content(message)),
        Role::System => return None,
    };

    let parts: Vec<Part> = message
        .content
        .iter()
        .filter_map(|block| to_part(block, message.role))
        .collect();

    if parts.is_empty() {
        return None;
    }

    Some(Content {
        role: role.to_string(),
        parts,
    })
}

fn to_part(block: &ContentBlock, _role: Role) -> Option<Part> {
    match block {
        ContentBlock::Text { text } => Some(Part::Text { text: text.clone() }),
        ContentBlock::ToolUse {
            id: _,
            name,
            input,
            signature,
        } => Some(Part::FunctionCall {
            function_call: FunctionCall {
                name: name.clone(),
                args: input.clone(),
            },
            thought_signature: signature.clone(),
        }),
        ContentBlock::Image { source } => match source {
            ImageSource::Base64 { media_type, data } => Some(Part::InlineData {
                inline_data: InlineData {
                    mime_type: media_type.clone(),
                    data: data.clone(),
                },
            }),
            ImageSource::Url { .. } => None,
        },
        ContentBlock::Thinking { .. }
        | ContentBlock::RedactedThinking { .. }
        | ContentBlock::ToolResult { .. } => None,
    }
}

fn to_tool_response_content(message: &Message) -> Content {
    let parts: Vec<Part> = message
        .content
        .iter()
        .filter_map(|block| {
            if let ContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
            } = block
            {
                let response_content = if is_error.unwrap_or(false) {
                    serde_json::json!({ "error": content })
                } else {
                    serde_json::json!({ "result": content })
                };

                Some(Part::FunctionResponse {
                    function_response: FunctionResponse {
                        name: tool_use_id.clone(),
                        response: FunctionResponseContent {
                            content: response_content,
                        },
                    },
                })
            } else {
                None
            }
        })
        .collect();

    Content {
        role: "user".to_string(),
        parts,
    }
}

fn to_function_declaration(tool: &ToolDefinition) -> FunctionDeclaration {
    let parameters = convert_to_gemini_schema(&tool.input_schema);

    FunctionDeclaration {
        name: tool.name.clone(),
        description: tool.description.clone(),
        parameters,
    }
}

const SUPPORTED_SCHEMA_FIELDS: &[&str] = &[
    "type",
    "nullable",
    "required",
    "format",
    "description",
    "properties",
    "items",
    "enum",
];

fn convert_to_gemini_schema(value: &serde_json::Value) -> serde_json::Value {
    convert_schema_internal(value, false)
}

fn convert_schema_internal(
    value: &serde_json::Value,
    is_properties_map: bool,
) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut result: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();

            for (key, val) in map {
                if is_properties_map {
                    result.insert(key.clone(), convert_schema_internal(val, false));
                    continue;
                }

                if !SUPPORTED_SCHEMA_FIELDS.contains(&key.as_str()) {
                    continue;
                }

                if key == "type" {
                    if let serde_json::Value::Array(types) = val {
                        let mut actual_type = None;
                        let mut is_nullable = false;

                        for t in types {
                            if let serde_json::Value::String(s) = t {
                                if s == "null" {
                                    is_nullable = true;
                                } else {
                                    actual_type = Some(s.clone());
                                }
                            }
                        }

                        if let Some(t) = actual_type {
                            result.insert("type".to_string(), serde_json::json!(t));
                        }
                        if is_nullable {
                            result.insert("nullable".to_string(), serde_json::json!(true));
                        }
                    } else {
                        result.insert(key.clone(), val.clone());
                    }
                } else if key == "properties" {
                    result.insert(key.clone(), convert_schema_internal(val, true));
                } else if key == "items" {
                    result.insert(key.clone(), convert_schema_internal(val, false));
                } else {
                    result.insert(key.clone(), val.clone());
                }
            }

            serde_json::Value::Object(result)
        }
        serde_json::Value::Array(arr) => serde_json::Value::Array(
            arr.iter()
                .map(|v| convert_schema_internal(v, false))
                .collect(),
        ),
        other => other.clone(),
    }
}

pub fn from_api_response(response: ApiResponse) -> CompletionResponse {
    let candidate = response.candidates.first();

    let (content_blocks, has_function_calls) = candidate
        .and_then(|c| c.content.as_ref())
        .map_or_else(|| (vec![], false), from_content);

    let message = Message {
        role: Role::Assistant,
        content: content_blocks,
    };

    let stop_reason = if has_function_calls {
        StopReason::ToolUse
    } else {
        candidate
            .and_then(|c| c.finish_reason.as_ref())
            .map_or(StopReason::EndTurn, |reason| match reason.as_str() {
                "STOP" => StopReason::EndTurn,
                "MAX_TOKENS" => StopReason::MaxTokens,
                "STOP_SEQUENCE" => StopReason::StopSequence,
                _ => StopReason::EndTurn,
            })
    };

    let usage = response.usage_metadata.map_or_else(Usage::default, |u| {
        Usage::new(u.prompt_token_count, u.candidates_token_count)
    });

    CompletionResponse::new(message, stop_reason, usage)
}

fn from_content(content: &Content) -> (Vec<ContentBlock>, bool) {
    let mut blocks = Vec::new();
    let mut has_function_calls = false;

    for part in &content.parts {
        match part {
            Part::Text { text } => {
                if !text.is_empty() {
                    blocks.push(ContentBlock::Text { text: text.clone() });
                }
            }
            Part::FunctionCall {
                function_call,
                thought_signature,
            } => {
                has_function_calls = true;
                blocks.push(ContentBlock::ToolUse {
                    id: Uuid::new_v4().to_string(),
                    name: function_call.name.clone(),
                    input: function_call.args.clone(),
                    signature: thought_signature.clone(),
                });
            }
            Part::InlineData { .. } | Part::FunctionResponse { .. } => {}
        }
    }

    (blocks, has_function_calls)
}

#[must_use]
pub fn parse_stream_event(data: &str) -> Option<CoreStreamEvent> {
    let response: ApiResponse = serde_json::from_str(data).ok()?;
    let candidate = response.candidates.first()?;

    if candidate.finish_reason.is_some() {
        let stop_reason = candidate
            .finish_reason
            .as_ref()
            .map_or(StopReason::EndTurn, |reason| match reason.as_str() {
                "STOP" => StopReason::EndTurn,
                "MAX_TOKENS" => StopReason::MaxTokens,
                "STOP_SEQUENCE" => StopReason::StopSequence,
                _ => StopReason::EndTurn,
            });

        return Some(CoreStreamEvent::MessageDelta {
            delta: MessageDelta {
                stop_reason: Some(stop_reason),
                usage: response
                    .usage_metadata
                    .map(|u| Usage::new(u.prompt_token_count, u.candidates_token_count)),
            },
        });
    }

    if let Some(content) = &candidate.content {
        for (idx, part) in content.parts.iter().enumerate() {
            match part {
                Part::Text { text } if !text.is_empty() => {
                    return Some(CoreStreamEvent::ContentBlockDelta {
                        index: idx,
                        delta: CoreContentDelta::TextDelta { text: text.clone() },
                    });
                }
                Part::FunctionCall {
                    function_call,
                    thought_signature,
                } => {
                    return Some(CoreStreamEvent::ContentBlockStart {
                        index: idx,
                        content_block: ContentBlock::ToolUse {
                            id: Uuid::new_v4().to_string(),
                            name: function_call.name.clone(),
                            input: function_call.args.clone(),
                            signature: thought_signature.clone(),
                        },
                    });
                }
                _ => {}
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::super::types::Candidate;
    use super::*;

    #[test]
    fn test_to_api_request_simple() {
        let request = CompletionRequest::new(vec![Message::user("Hello")]);
        let api_request = to_api_request(&request);

        assert_eq!(api_request.contents.len(), 1);
        assert_eq!(api_request.contents[0].role, "user");
    }

    #[test]
    fn test_to_api_request_with_system() {
        let request =
            CompletionRequest::new(vec![Message::user("Hello")]).with_system_prompt("Be helpful");
        let api_request = to_api_request(&request);

        assert!(api_request.system_instruction.is_some());
    }

    #[test]
    fn test_to_api_request_with_tools() {
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
        let api_request = to_api_request(&request);

        assert!(api_request.tools.is_some());
        assert_eq!(api_request.tools.unwrap()[0].function_declarations.len(), 1);
    }

    #[test]
    fn test_from_api_response() {
        let response = ApiResponse {
            candidates: vec![Candidate {
                content: Some(Content {
                    role: "model".to_string(),
                    parts: vec![Part::Text {
                        text: "Hello!".to_string(),
                    }],
                }),
                finish_reason: Some("STOP".to_string()),
                safety_ratings: vec![],
                index: 0,
            }],
            usage_metadata: None,
            prompt_feedback: None,
        };

        let completion = from_api_response(response);
        assert_eq!(completion.message.first_text(), Some("Hello!"));
        assert_eq!(completion.stop_reason, StopReason::EndTurn);
    }

    #[test]
    fn test_from_api_response_with_function_call() {
        let response = ApiResponse {
            candidates: vec![Candidate {
                content: Some(Content {
                    role: "model".to_string(),
                    parts: vec![Part::FunctionCall {
                        function_call: FunctionCall {
                            name: "read_file".to_string(),
                            args: serde_json::json!({"path": "/tmp/test.txt"}),
                        },
                        thought_signature: Some("test_signature".to_string()),
                    }],
                }),
                finish_reason: Some("STOP".to_string()),
                safety_ratings: vec![],
                index: 0,
            }],
            usage_metadata: None,
            prompt_feedback: None,
        };

        let completion = from_api_response(response);
        assert!(completion.message.has_tool_use());
        assert_eq!(completion.stop_reason, StopReason::ToolUse);
    }

    #[test]
    fn test_parse_stream_event_text() {
        let data =
            r#"{"candidates":[{"content":{"role":"model","parts":[{"text":"Hello"}]},"index":0}]}"#;
        let event = parse_stream_event(data);

        assert!(event.is_some());
        if let Some(CoreStreamEvent::ContentBlockDelta { delta, .. }) = event {
            if let CoreContentDelta::TextDelta { text } = delta {
                assert_eq!(text, "Hello");
            } else {
                panic!("Expected TextDelta");
            }
        } else {
            panic!("Expected ContentBlockDelta");
        }
    }
}
