use bytes::Bytes;
use futures::stream::{Stream, StreamExt};

use crate::providers::error::ProviderError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseEvent {
    pub event_type: Option<String>,
    pub data: String,
}

impl SseEvent {
    #[must_use]
    pub fn new(data: impl Into<String>) -> Self {
        Self {
            event_type: None,
            data: data.into(),
        }
    }

    #[must_use]
    pub fn with_type(event_type: impl Into<String>, data: impl Into<String>) -> Self {
        Self {
            event_type: Some(event_type.into()),
            data: data.into(),
        }
    }
}

pub struct SseParser {
    buffer: String,
    current_event_type: Option<String>,
    data_lines: Vec<String>,
}

impl SseParser {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            buffer: String::new(),
            current_event_type: None,
            data_lines: Vec::new(),
        }
    }

    pub fn process_chunk(&mut self, chunk: &[u8]) -> Vec<SseEvent> {
        let text = String::from_utf8_lossy(chunk);
        self.buffer.push_str(&text);

        let mut events = Vec::new();

        while let Some(line_end) = self.buffer.find('\n') {
            let line = self.buffer[..line_end].trim_end_matches('\r').to_string();
            self.buffer = self.buffer[line_end + 1..].to_string();

            if line.is_empty() {
                if !self.data_lines.is_empty() {
                    let data = self.data_lines.join("\n");
                    events.push(SseEvent {
                        event_type: self.current_event_type.take(),
                        data,
                    });
                    self.data_lines.clear();
                }
            } else if let Some(event_type) = line.strip_prefix("event:") {
                self.current_event_type = Some(event_type.trim().to_string());
            } else if let Some(data) = line.strip_prefix("data:") {
                let data = data.trim_start();
                if data != "[DONE]" {
                    self.data_lines.push(data.to_string());
                }
            }
        }

        events
    }

    pub fn parse_stream<S>(byte_stream: S) -> impl Stream<Item = Result<SseEvent, ProviderError>>
    where
        S: Stream<Item = Result<Bytes, reqwest::Error>> + Unpin,
    {
        let parser = std::sync::Arc::new(std::sync::Mutex::new(Self::new()));

        byte_stream.flat_map(move |result: Result<Bytes, reqwest::Error>| {
            let parser = parser.clone();
            let events: Vec<Result<SseEvent, ProviderError>> = match result {
                Ok(bytes) => {
                    let parsed = parser
                        .lock()
                        .map(|mut p| p.process_chunk(&bytes))
                        .unwrap_or_default();
                    parsed.into_iter().map(Ok).collect()
                }
                Err(e) => {
                    vec![Err(ProviderError::StreamError(e.to_string()))]
                }
            };
            futures::stream::iter(events)
        })
    }
}

impl Default for SseParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_data_event() {
        let mut parser = SseParser::new();
        let events = parser.process_chunk(b"data: hello world\n\n");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "hello world");
        assert!(events[0].event_type.is_none());
    }

    #[test]
    fn test_event_with_type() {
        let mut parser = SseParser::new();
        let events = parser.process_chunk(b"event: message\ndata: hello\n\n");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "hello");
        assert_eq!(events[0].event_type, Some("message".to_string()));
    }

    #[test]
    fn test_multi_line_data() {
        let mut parser = SseParser::new();
        let events = parser.process_chunk(b"data: line1\ndata: line2\ndata: line3\n\n");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "line1\nline2\nline3");
    }

    #[test]
    fn test_multiple_events() {
        let mut parser = SseParser::new();
        let events = parser.process_chunk(b"data: first\n\ndata: second\n\n");

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].data, "first");
        assert_eq!(events[1].data, "second");
    }

    #[test]
    fn test_chunked_input() {
        let mut parser = SseParser::new();

        let events1 = parser.process_chunk(b"data: hel");
        assert!(events1.is_empty());

        let events2 = parser.process_chunk(b"lo world\n\n");
        assert_eq!(events2.len(), 1);
        assert_eq!(events2[0].data, "hello world");
    }

    #[test]
    fn test_done_sentinel_ignored() {
        let mut parser = SseParser::new();
        let events = parser.process_chunk(b"data: [DONE]\n\n");

        assert!(events.is_empty());
    }

    #[test]
    fn test_json_data() {
        let mut parser = SseParser::new();
        let json = r#"data: {"type": "message", "content": "hello"}"#;
        let events = parser.process_chunk(format!("{json}\n\n").as_bytes());

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, r#"{"type": "message", "content": "hello"}"#);
    }

    #[test]
    fn test_carriage_return_handling() {
        let mut parser = SseParser::new();
        let events = parser.process_chunk(b"data: hello\r\n\r\n");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "hello");
    }

    #[test]
    fn test_anthropic_style_events() {
        let mut parser = SseParser::new();
        let input = b"event: content_block_delta\ndata: {\"type\":\"content_block_delta\"}\n\n";
        let events = parser.process_chunk(input);

        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].event_type,
            Some("content_block_delta".to_string())
        );
    }

    #[test]
    fn test_empty_input() {
        let mut parser = SseParser::new();
        let events = parser.process_chunk(b"");
        assert!(events.is_empty());
    }

    #[test]
    fn test_no_trailing_newline_buffers() {
        let mut parser = SseParser::new();

        let events1 = parser.process_chunk(b"data: waiting\n");
        assert!(events1.is_empty());

        let events2 = parser.process_chunk(b"\n");
        assert_eq!(events2.len(), 1);
        assert_eq!(events2[0].data, "waiting");
    }
}
