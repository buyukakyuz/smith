pub mod event_handler;
pub mod models;
pub mod persistence;

use config::{Config, Environment, File};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::{fs, io};

pub use event_handler::{ConfigEvent, ConfigEventHandler, ConfigEventSender};
pub use models::ModelRegistry;
pub use persistence::{ConfigError, ConfigPatch, ConfigPersister, ConfigResult};

pub fn get_config_dir() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|h| h.join("Library/Application Support/smith"))
    }

    #[cfg(target_os = "linux")]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .map(|c| c.join("smith"))
    }

    #[cfg(target_os = "windows")]
    {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .map(|a| a.join("smith"))
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|h| h.join(".config/smith"))
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub custom_system_prompt: Option<String>,
}

impl AppConfig {
    #[must_use]
    pub fn load() -> Self {
        let mut builder = Config::builder();

        if let Some(path) = Self::get_config_path() {
            builder = builder.add_source(File::from(path).required(false));
        }

        builder = builder.add_source(Environment::with_prefix("SMITH"));

        builder
            .build()
            .and_then(Config::try_deserialize)
            .unwrap_or_else(|e| {
                eprintln!("Warning: Failed to load config: {e}");
                Self::default()
            })
    }

    #[must_use]
    pub fn get_config_path() -> Option<PathBuf> {
        get_config_dir().map(|dir| dir.join("config.toml"))
    }

    pub fn init_default() -> Result<PathBuf, io::Error> {
        let path = Self::get_config_path().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "Could not determine config directory",
            )
        })?;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        if path.exists() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("Config file already exists at {}", path.display()),
            ));
        }

        fs::write(&path, include_str!("config.template.toml"))?;
        Ok(path)
    }
}
