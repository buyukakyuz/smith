pub mod agent_runner;
pub mod app;
pub mod events;
pub mod layout;
pub mod permission_ui;
pub mod state;
pub mod widgets;

pub use app::TuiApp;
pub use events::TuiToolEventHandler;
pub use permission_ui::TuiPermissionUI;

use crate::config::{ConfigEventHandler, ConfigPersister};
use crate::core::augmented_llm::AugmentedLLM;
use crate::core::error::Result;
use crate::permission::PermissionManager;
use std::sync::Arc;
use tokio::sync::mpsc;

pub async fn run_tui(mut agent: AugmentedLLM, show_model_picker: bool) -> Result<()> {
    let (event_tx, event_rx) = mpsc::unbounded_channel();

    let config_event_tx = if let Some(persister) = ConfigPersister::with_default_path() {
        let (handler, tx) = ConfigEventHandler::new(Arc::new(persister));
        tokio::spawn(handler.run());
        Some(tx)
    } else {
        tracing::warn!("Could not determine config path, model selection will not be persisted");
        None
    };

    agent.register_tool_event_handler(Arc::new(TuiToolEventHandler::new(event_tx.clone())));

    let permission_ui = Arc::new(TuiPermissionUI::new(event_tx.clone()));
    let permission_manager = PermissionManager::new()?.with_ui(permission_ui);
    agent.set_permission_manager(Arc::new(permission_manager));

    let mut app = TuiApp::with_event_channels(
        agent,
        event_tx,
        event_rx,
        config_event_tx,
        show_model_picker,
    )?;
    app.run().await
}
