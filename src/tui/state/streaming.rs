use super::AppState;

impl AppState {
    pub fn append_streaming(&mut self, chunk: &str) {
        match &mut self.streaming_response {
            Some(existing) => existing.push_str(chunk),
            None => self.streaming_response = Some(chunk.to_string()),
        }

        if !self.scroll.is_manual_scroll() {
            self.scroll.scroll_to_bottom();
        }
    }

    pub fn finalize_streaming(&mut self) -> String {
        self.streaming_response.take().unwrap_or_default()
    }

    #[must_use]
    pub fn is_streaming(&self) -> bool {
        self.streaming_response.is_some()
    }
}
