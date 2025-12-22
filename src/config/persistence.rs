use crate::config::{AppConfig, get_config_dir};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::PathBuf;

pub type ConfigResult<T> = std::result::Result<T, ConfigError>;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("TOML serialization error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),

    #[error("TOML parse error: {0}")]
    TomlParse(#[from] toml::de::Error),

    #[error("Config directory not found")]
    NoConfigDir,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConfigPatch {
    pub provider: Option<String>,
    pub model: Option<String>,
}

impl ConfigPatch {
    #[must_use]
    pub fn model(provider: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            provider: Some(provider.into()),
            model: Some(model.into()),
            ..Default::default()
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.provider.is_none() && self.model.is_none()
    }
}

pub struct ConfigPersister {
    config_path: PathBuf,
    write_lock: Mutex<()>,
}

impl ConfigPersister {
    #[must_use]
    pub fn new(config_path: PathBuf) -> Self {
        Self {
            config_path,
            write_lock: Mutex::new(()),
        }
    }

    #[must_use]
    pub fn with_default_path() -> Option<Self> {
        get_config_dir().map(|dir| Self::new(dir.join("config.toml")))
    }

    pub fn apply_patch(&self, patch: &ConfigPatch) -> ConfigResult<()> {
        if patch.is_empty() {
            return Ok(());
        }

        let _lock = self.write_lock.lock();

        let existing = self.read_existing_config()?;

        let merged = Self::merge_config(existing, patch);

        self.atomic_write(&merged)
    }

    fn read_existing_config(&self) -> ConfigResult<AppConfig> {
        if self.config_path.exists() {
            let content = fs::read_to_string(&self.config_path)?;
            let config: AppConfig = toml::from_str(&content)?;
            Ok(config)
        } else {
            Ok(AppConfig::default())
        }
    }

    fn merge_config(mut existing: AppConfig, patch: &ConfigPatch) -> AppConfig {
        if let Some(ref provider) = patch.provider {
            existing.provider = Some(provider.clone());
        }
        if let Some(ref model) = patch.model {
            existing.model = Some(model.clone());
        }
        existing
    }

    fn atomic_write(&self, config: &AppConfig) -> ConfigResult<()> {
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let toml_content = toml::to_string_pretty(config)?;
        let content = format!(
            "# Smith Configuration\n\
             # This file is automatically managed by smith.\n\n\
             {toml_content}"
        );

        let temp_path = self.config_path.with_extension("toml.tmp");
        fs::write(&temp_path, &content)?;

        fs::rename(&temp_path, &self.config_path)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_config_patch_model() {
        let patch = ConfigPatch::model("anthropic", "claude-sonnet-4");
        assert_eq!(patch.provider, Some("anthropic".to_string()));
        assert_eq!(patch.model, Some("claude-sonnet-4".to_string()));
    }

    #[test]
    fn test_config_patch_is_empty() {
        let empty = ConfigPatch::default();
        assert!(empty.is_empty());

        let non_empty = ConfigPatch::model("anthropic", "claude-sonnet-4");
        assert!(!non_empty.is_empty());
    }

    #[test]
    fn test_persister_apply_patch() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config_path = temp_dir.path().join("config.toml");

        let persister = ConfigPersister::new(config_path.clone());

        let patch = ConfigPatch::model("anthropic", "claude-sonnet-4");
        persister
            .apply_patch(&patch)
            .expect("Failed to apply patch");

        let content = fs::read_to_string(&config_path).expect("Failed to read config");
        assert!(content.contains("provider = \"anthropic\""));
        assert!(content.contains("model = \"claude-sonnet-4\""));

        let patch2 = ConfigPatch {
            model: Some("claude-opus-4-5".to_string()),
            ..Default::default()
        };
        persister
            .apply_patch(&patch2)
            .expect("Failed to apply second patch");

        let content = fs::read_to_string(&config_path).expect("Failed to read config");
        assert!(content.contains("provider = \"anthropic\""));
        assert!(content.contains("model = \"claude-opus-4-5\""));
    }

    #[test]
    fn test_persister_empty_patch_noop() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config_path = temp_dir.path().join("config.toml");

        let persister = ConfigPersister::new(config_path.clone());

        let empty = ConfigPatch::default();
        persister
            .apply_patch(&empty)
            .expect("Failed to apply empty patch");

        assert!(!config_path.exists());
    }
}
