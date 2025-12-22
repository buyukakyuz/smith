use crate::core::types::{ContentBlock, ContentDelta};
use std::collections::HashMap;
#[derive(Default)]
pub(super) struct StreamAccumulator {
    blocks: HashMap<usize, ContentBlock>,
    tool_inputs: HashMap<usize, String>,
}

impl StreamAccumulator {
    pub fn handle_block_start(&mut self, index: usize, content_block: ContentBlock) {
        self.blocks.insert(index, content_block);
    }

    pub fn handle_delta(&mut self, index: usize, delta: ContentDelta) {
        match delta {
            ContentDelta::TextDelta { text } => self.append_text(index, &text),
            ContentDelta::ThinkingDelta { thinking } => self.append_thinking(index, &thinking),
            ContentDelta::SignatureDelta { signature } => self.append_signature(index, &signature),
            ContentDelta::InputJsonDelta { partial_json } => {
                self.tool_inputs
                    .entry(index)
                    .or_default()
                    .push_str(&partial_json);
            }
        }
    }

    pub fn into_content_blocks(mut self) -> Vec<ContentBlock> {
        self.merge_tool_inputs();
        self.into_sorted_blocks()
    }

    fn append_text(&mut self, index: usize, text: &str) {
        if let Some(ContentBlock::Text { text: buf }) = self.blocks.get_mut(&index) {
            buf.push_str(text);
        }
    }

    fn append_thinking(&mut self, index: usize, thinking: &str) {
        if let Some(ContentBlock::Thinking { thinking: buf, .. }) = self.blocks.get_mut(&index) {
            buf.push_str(thinking);
        }
    }

    fn append_signature(&mut self, index: usize, signature: &str) {
        if let Some(ContentBlock::Thinking {
            signature: sig_slot,
            ..
        }) = self.blocks.get_mut(&index)
        {
            match sig_slot {
                Some(buf) => buf.push_str(signature),
                None => *sig_slot = Some(signature.to_owned()),
            }
        }
    }

    fn merge_tool_inputs(&mut self) {
        for (index, json) in self.tool_inputs.drain() {
            let Some(ContentBlock::ToolUse { input, .. }) = self.blocks.get_mut(&index) else {
                continue;
            };

            match serde_json::from_str(&json) {
                Ok(parsed) => *input = parsed,
                Err(e) => {
                    tracing::warn!(
                        index,
                        error = %e,
                        json_preview = %truncate(&json, 100),
                        "Failed to parse tool input JSON"
                    );
                }
            }
        }
    }

    fn into_sorted_blocks(self) -> Vec<ContentBlock> {
        let mut entries: Vec<_> = self.blocks.into_iter().collect();
        entries.sort_unstable_by_key(|(idx, _)| *idx);
        entries.into_iter().map(|(_, block)| block).collect()
    }
}

fn truncate(s: &str, max_len: usize) -> &str {
    if s.len() <= max_len { s } else { &s[..max_len] }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accumulates_text_deltas() {
        let mut acc = StreamAccumulator::default();
        acc.handle_block_start(
            0,
            ContentBlock::Text {
                text: String::new(),
            },
        );
        acc.handle_delta(
            0,
            ContentDelta::TextDelta {
                text: "Hello".into(),
            },
        );
        acc.handle_delta(
            0,
            ContentDelta::TextDelta {
                text: " world".into(),
            },
        );

        let blocks = acc.into_content_blocks();

        assert_eq!(blocks.len(), 1);
        assert!(matches!(&blocks[0], ContentBlock::Text { text } if text == "Hello world"));
    }

    #[test]
    fn handles_sparse_indices() {
        let mut acc = StreamAccumulator::default();
        acc.handle_block_start(
            0,
            ContentBlock::Text {
                text: "first".into(),
            },
        );
        acc.handle_block_start(
            5,
            ContentBlock::Text {
                text: "second".into(),
            },
        );
        acc.handle_block_start(
            2,
            ContentBlock::Text {
                text: "middle".into(),
            },
        );

        let blocks = acc.into_content_blocks();

        assert_eq!(blocks.len(), 3);
        assert!(matches!(&blocks[0], ContentBlock::Text { text } if text == "first"));
        assert!(matches!(&blocks[1], ContentBlock::Text { text } if text == "middle"));
        assert!(matches!(&blocks[2], ContentBlock::Text { text } if text == "second"));
    }

    #[test]
    fn ignores_delta_for_missing_block() {
        let mut acc = StreamAccumulator::default();

        acc.handle_delta(
            0,
            ContentDelta::TextDelta {
                text: "orphan".into(),
            },
        );

        let blocks = acc.into_content_blocks();
        assert!(blocks.is_empty());
    }
}
