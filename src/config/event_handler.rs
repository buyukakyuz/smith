use crate::config::persistence::{ConfigError, ConfigPatch, ConfigPersister};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::warn;

#[derive(Debug, Clone)]
pub enum ConfigEvent {
    ModelChanged { provider: String, model: String },
}

pub type ConfigEventSender = mpsc::UnboundedSender<ConfigEvent>;
pub type ConfigEventReceiver = mpsc::UnboundedReceiver<ConfigEvent>;

pub struct ConfigEventHandler {
    persister: Arc<ConfigPersister>,
    event_rx: ConfigEventReceiver,
}

impl ConfigEventHandler {
    #[must_use]
    pub fn new(persister: Arc<ConfigPersister>) -> (Self, ConfigEventSender) {
        let (tx, rx) = mpsc::unbounded_channel();
        (
            Self {
                persister,
                event_rx: rx,
            },
            tx,
        )
    }

    pub async fn run(mut self) {
        while let Some(event) = self.event_rx.recv().await {
            if let Err(e) = self.handle_event(&event) {
                warn!(
                    "Failed to persist config change: {}. Change succeeded in-memory.",
                    e
                );
            }
        }
    }

    fn handle_event(&self, event: &ConfigEvent) -> Result<(), ConfigError> {
        match event {
            ConfigEvent::ModelChanged { provider, model } => {
                let patch = ConfigPatch::model(provider, model);
                self.persister.apply_patch(&patch)?;
                tracing::debug!("Persisted model change: {}/{}", provider, model);
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_config_event_handler() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config_path = temp_dir.path().join("config.toml");
        let persister = Arc::new(ConfigPersister::new(config_path.clone()));

        let (handler, tx) = ConfigEventHandler::new(persister);

        let handle = tokio::spawn(handler.run());

        tx.send(ConfigEvent::ModelChanged {
            provider: "anthropic".to_string(),
            model: "claude-sonnet-4".to_string(),
        })
        .expect("Failed to send event");

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        drop(tx);

        let _ = handle.await;

        let content = std::fs::read_to_string(&config_path).expect("Failed to read config");
        assert!(content.contains("provider = \"anthropic\""));
        assert!(content.contains("model = \"claude-sonnet-4\""));
    }
}
