#[derive(Debug, Clone)]
pub struct ScrollState {
    position: usize,
    total_lines: usize,
    viewport_height: usize,
    manual_scroll: bool,
}

impl ScrollState {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            position: 0,
            total_lines: 0,
            viewport_height: 0,
            manual_scroll: false,
        }
    }

    #[must_use]
    pub const fn position(&self) -> usize {
        self.position
    }

    #[must_use]
    pub const fn is_manual_scroll(&self) -> bool {
        self.manual_scroll
    }

    #[must_use]
    pub const fn is_at_bottom(&self) -> bool {
        if self.total_lines <= self.viewport_height {
            true
        } else {
            self.position >= self.max_scroll()
        }
    }

    pub fn update(&mut self, total_lines: usize, viewport_height: usize) {
        self.total_lines = total_lines;
        self.viewport_height = viewport_height;
        self.position = self.position.min(self.max_scroll());
    }

    pub const fn scroll_to_bottom(&mut self) {
        self.position = self.max_scroll();
        self.manual_scroll = false;
    }

    pub const fn scroll_to_top(&mut self) {
        self.position = 0;
        self.manual_scroll = true;
    }

    pub const fn scroll_up(&mut self, lines: usize) {
        self.position = self.position.saturating_sub(lines);
        self.manual_scroll = true;
    }

    pub fn scroll_down(&mut self, lines: usize) {
        self.position = (self.position + lines).min(self.max_scroll());
        self.manual_scroll = true;
    }

    pub const fn reset_manual_scroll(&mut self) {
        self.manual_scroll = false;
    }

    const fn max_scroll(&self) -> usize {
        self.total_lines.saturating_sub(self.viewport_height)
    }
}

impl Default for ScrollState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_at_top() {
        let state = ScrollState::new();
        assert_eq!(state.position(), 0);
        assert!(!state.is_manual_scroll());
    }

    #[test]
    fn is_at_bottom_when_content_fits() {
        let mut state = ScrollState::new();
        state.update(5, 10);
        assert!(state.is_at_bottom());
    }

    #[test]
    fn is_at_bottom_after_scroll_to_bottom() {
        let mut state = ScrollState::new();
        state.update(20, 10);

        assert!(!state.is_at_bottom());

        state.scroll_to_bottom();
        assert!(state.is_at_bottom());
        assert!(!state.is_manual_scroll());
    }

    #[test]
    fn scroll_up_down() {
        let mut state = ScrollState::new();
        state.update(20, 10);

        state.scroll_down(5);
        assert_eq!(state.position(), 5);
        assert!(state.is_manual_scroll());

        state.scroll_up(2);
        assert_eq!(state.position(), 3);
    }

    #[test]
    fn clamps_to_bounds() {
        let mut state = ScrollState::new();
        state.update(20, 10);

        state.scroll_down(100);
        assert_eq!(state.position(), 10);

        state.scroll_up(100);
        assert_eq!(state.position(), 0);
    }

    #[test]
    fn update_clamps_existing_position() {
        let mut state = ScrollState::new();
        state.update(100, 10);
        state.scroll_down(50);

        state.update(20, 10);
        assert_eq!(state.position(), 10);
    }
}
