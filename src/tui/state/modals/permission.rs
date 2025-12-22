use crate::permission::types::{PermissionRequest, PermissionResponse};
use tokio::sync::oneshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Selection {
    AllowOnce = 0,
    AllowSession = 1,
    Deny = 2,
}

impl Selection {
    fn from_index(index: usize) -> Self {
        match index {
            0 => Self::AllowOnce,
            1 => Self::AllowSession,
            _ => Self::Deny,
        }
    }
}

pub struct PermissionModal {
    pub request: PermissionRequest,
    selected: usize,
    response_tx: oneshot::Sender<PermissionResponse>,
    input_mode: bool,
    feedback_input: String,
}

impl PermissionModal {
    pub fn new(
        request: PermissionRequest,
        response_tx: oneshot::Sender<PermissionResponse>,
    ) -> Self {
        Self {
            request,
            selected: 0,
            response_tx,
            input_mode: false,
            feedback_input: String::new(),
        }
    }

    #[must_use]
    pub fn selected_index(&self) -> usize {
        self.selected
    }

    #[must_use]
    pub fn is_input_mode(&self) -> bool {
        self.input_mode
    }

    #[must_use]
    pub fn feedback(&self) -> &str {
        &self.feedback_input
    }

    pub fn input_char(&mut self, c: char) {
        if self.input_mode {
            self.feedback_input.push(c);
        }
    }

    pub fn input_backspace(&mut self) {
        if self.input_mode {
            self.feedback_input.pop();
        }
    }

    pub fn set_selection(&mut self, index: usize) {
        self.selected = index.min(2);
        self.update_input_mode();
    }

    pub fn select_prev(&mut self) {
        self.selected = self.selected.saturating_sub(1);
        self.update_input_mode();
    }

    pub fn select_next(&mut self) {
        self.selected = (self.selected + 1).min(2);
        self.update_input_mode();
    }

    fn update_input_mode(&mut self) {
        let is_deny = self.selected == Selection::Deny as usize;
        self.input_mode = is_deny;

        if !is_deny {
            self.feedback_input.clear();
        }
    }

    pub fn confirm(self) -> Option<String> {
        let (response, feedback) = match Selection::from_index(self.selected) {
            Selection::AllowOnce => (PermissionResponse::AllowOnce, None),
            Selection::AllowSession => (PermissionResponse::AllowSession, None),
            Selection::Deny => {
                let feedback = if self.feedback_input.trim().is_empty() {
                    "User declined the operation. Please ask what to do instead.".to_string()
                } else {
                    self.feedback_input.clone()
                };
                (
                    PermissionResponse::TellModelDifferently(feedback.clone()),
                    Some(feedback),
                )
            }
        };

        let _ = self.response_tx.send(response);
        feedback
    }

    pub fn cancel(self) {
        let _ = self
            .response_tx
            .send(PermissionResponse::TellModelDifferently(
                "User cancelled the operation".to_string(),
            ));
    }
}
