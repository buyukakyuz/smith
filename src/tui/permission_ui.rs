use crate::core::error::Result;
use crate::permission::types::{PermissionRequest, PermissionResponse};
use crate::permission::ui_trait::PermissionUI;
use crate::tui::events::AppEvent;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::oneshot;

pub struct TuiPermissionUI {
    event_tx: UnboundedSender<AppEvent>,
}

impl TuiPermissionUI {
    #[must_use]
    pub const fn new(event_tx: UnboundedSender<AppEvent>) -> Self {
        Self { event_tx }
    }
}

impl PermissionUI for TuiPermissionUI {
    fn prompt_user(&self, request: &PermissionRequest) -> Result<PermissionResponse> {
        let (response_tx, response_rx) = oneshot::channel();

        self.event_tx
            .send(AppEvent::PermissionRequired {
                request: request.clone(),
                response_tx,
            })
            .map_err(|e| {
                crate::core::error::AgentError::InvalidState(format!(
                    "Failed to send permission request to TUI: {e}"
                ))
            })?;

        let response =
            tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(response_rx))
                .map_err(|e| {
                    crate::core::error::AgentError::InvalidState(format!(
                        "Permission response channel closed: {e}"
                    ))
                })?;

        Ok(response)
    }
}
