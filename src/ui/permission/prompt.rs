use crossterm::event::{self, Event};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io::{self, stdout};

use super::input::{KeyAction, Selection, handle_key};
use super::render::PromptRenderer;
use crate::permission::types::{PermissionRequest, PermissionResponse};
use crate::tui::app::TerminalGuard;

pub struct PermissionPrompt {
    request: PermissionRequest,
    selected: Selection,
    content_preview: Option<String>,
}

impl PermissionPrompt {
    #[must_use]
    pub fn new(request: PermissionRequest) -> Self {
        Self {
            request,
            selected: Selection::AllowOnce,
            content_preview: None,
        }
    }

    #[must_use]
    pub fn with_preview(mut self, content: impl Into<String>) -> Self {
        self.content_preview = Some(content.into());
        self
    }

    pub fn run(&mut self) -> io::Result<PermissionResponse> {
        let _guard = TerminalGuard::acquire()?;
        let backend = CrosstermBackend::new(stdout());
        let mut terminal = Terminal::new(backend)?;

        self.event_loop(&mut terminal)
    }

    fn event_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> io::Result<PermissionResponse> {
        loop {
            terminal.draw(|frame| {
                let renderer = PromptRenderer::new(
                    &self.request,
                    self.selected,
                    self.content_preview.as_deref(),
                );
                renderer.draw(frame);
            })?;

            if let Event::Key(key) = event::read()? {
                match handle_key(key, self.selected) {
                    KeyAction::Navigate(new_selection) => self.selected = new_selection,
                    KeyAction::Confirm(response) => return Ok(response),
                    KeyAction::None => {}
                }
            }
        }
    }
}

pub fn prompt_user(request: &PermissionRequest) -> io::Result<PermissionResponse> {
    PermissionPrompt::new(request.clone()).run()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permission::types::PermissionType;

    #[test]
    fn test_permission_prompt_creation() {
        let request = PermissionRequest::new(PermissionType::FileWrite, "test.txt");
        let prompt = PermissionPrompt::new(request);
        assert_eq!(prompt.selected, Selection::AllowOnce);
    }

    #[test]
    fn test_selection_navigation() {
        assert_eq!(Selection::AllowOnce.next(), Selection::AllowSession);
        assert_eq!(Selection::AllowSession.next(), Selection::Feedback);
        assert_eq!(Selection::Feedback.next(), Selection::Feedback);

        assert_eq!(Selection::Feedback.prev(), Selection::AllowSession);
        assert_eq!(Selection::AllowSession.prev(), Selection::AllowOnce);
        assert_eq!(Selection::AllowOnce.prev(), Selection::AllowOnce);
    }
}
