use crate::tools::{ToolState, ToolType};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct ToolCard {
    pub(crate) tool_type: ToolType,
    pub(crate) state: ToolState,
    pub(crate) input_summary: String,
    pub(crate) output_lines: Vec<String>,
    pub(crate) elapsed: Option<Duration>,
    pub(crate) frame_index: usize,
    pub(crate) show_preview: bool,
    pub(crate) max_preview_lines: usize,
}

impl ToolCard {
    #[must_use]
    pub fn new(tool_type: ToolType, input_summary: impl Into<String>) -> Self {
        Self {
            tool_type,
            state: ToolState::Starting,
            input_summary: input_summary.into(),
            output_lines: Vec::new(),
            elapsed: None,
            frame_index: 0,
            show_preview: true,
            max_preview_lines: 5,
        }
    }

    #[must_use]
    pub const fn state(mut self, state: ToolState) -> Self {
        self.state = state;
        self
    }

    #[must_use]
    pub fn output(mut self, lines: Vec<String>) -> Self {
        self.output_lines = lines;
        self
    }

    #[must_use]
    pub const fn elapsed(mut self, elapsed: Duration) -> Self {
        self.elapsed = Some(elapsed);
        self
    }

    #[must_use]
    pub const fn frame_index(mut self, index: usize) -> Self {
        self.frame_index = index;
        self
    }

    pub(crate) fn should_show_preview(&self) -> bool {
        self.show_preview && !self.output_lines.is_empty() && self.state == ToolState::Success
    }
}
