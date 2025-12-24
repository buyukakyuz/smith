pub mod agent_runner;
pub mod app;
pub mod events;
pub mod layout;
pub mod permission_ui;
pub mod state;
pub mod widgets;

pub use agent_runner::AgentConfig;
pub use app::TuiApp;
pub use events::TuiToolEventHandler;
pub use permission_ui::TuiPermissionUI;

use crate::config::{ConfigEventHandler, ConfigPersister};
use crate::core::error::Result;
use std::sync::Arc;
use tokio::sync::mpsc;

pub async fn run_tui(agent_config: AgentConfig, show_model_picker: bool) -> Result<()> {
    let (event_tx, event_rx) = mpsc::unbounded_channel();

    let config_event_tx = ConfigPersister::with_default_path().map_or_else(
        || {
            tracing::warn!(
                "Could not determine config path, model selection will not be persisted"
            );
            None
        },
        |persister| {
            let (handler, tx) = ConfigEventHandler::new(Arc::new(persister));
            tokio::spawn(handler.run());
            Some(tx)
        },
    );

    let mut app = TuiApp::with_lazy_agent(
        agent_config,
        event_tx,
        event_rx,
        config_event_tx,
        show_model_picker,
    )?;
    app.run().await
}
