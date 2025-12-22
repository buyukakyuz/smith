use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::types::{Pattern, PatternError, PermissionType};
use crate::core::error::{AgentError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionConfig {
    #[serde(default)]
    pub allowed_commands: Vec<Pattern>,

    #[serde(default)]
    pub allowed_write_paths: Vec<Pattern>,

    #[serde(default)]
    pub allowed_delete_paths: Vec<Pattern>,

    #[serde(default)]
    pub allowed_network_hosts: Vec<Pattern>,

    #[serde(default)]
    pub custom_permissions: HashMap<String, Vec<Pattern>>,

    pub created_at: DateTime<Utc>,

    pub last_updated: DateTime<Utc>,
}

impl Default for PermissionConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl PermissionConfig {
    #[must_use]
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            allowed_commands: Vec::new(),
            allowed_write_paths: Vec::new(),
            allowed_delete_paths: Vec::new(),
            allowed_network_hosts: Vec::new(),
            custom_permissions: HashMap::new(),
            created_at: now,
            last_updated: now,
        }
    }

    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = serde_json::from_str(&content)?;
        Ok(config)
    }

    pub fn matches_any_pattern(&self, target: &str, patterns: &[Pattern]) -> Result<bool> {
        for pattern in patterns {
            if pattern.matches(target).map_err(|e: PatternError| {
                AgentError::Config(format!("Invalid pattern '{pattern}': {e}"))
            })? {
                return Ok(true);
            }
        }
        Ok(false)
    }
    pub fn is_allowed(&self, perm_type: PermissionType, target: &str) -> Result<bool> {
        match perm_type {
            PermissionType::FileRead => Ok(true),
            PermissionType::FileWrite => {
                self.matches_any_pattern(target, &self.allowed_write_paths)
            }
            PermissionType::FileDelete => {
                self.matches_any_pattern(target, &self.allowed_delete_paths)
            }
            PermissionType::CommandExecute => {
                self.matches_any_pattern(target, &self.allowed_commands)
            }
            PermissionType::NetworkAccess => {
                self.matches_any_pattern(target, &self.allowed_network_hosts)
            }
            PermissionType::SystemModification => Ok(false),
        }
    }

    #[must_use]
    pub fn default_config_dir() -> PathBuf {
        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let local_config = current_dir.join(".smith");

        if local_config.exists() {
            return local_config;
        }

        crate::config::get_config_dir().unwrap_or_else(|| PathBuf::from(".smith"))
    }

    #[must_use]
    pub fn default_permissions_file() -> PathBuf {
        Self::default_config_dir().join("permissions.json")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_config() {
        let config = PermissionConfig::new();
        assert!(config.allowed_commands.is_empty());
    }

    #[test]
    fn test_matches_any_pattern() {
        let config = PermissionConfig::new();
        let patterns = vec![
            Pattern::Exact("test.txt".to_string()),
            Pattern::Glob("*.rs".to_string()),
        ];

        assert!(config.matches_any_pattern("test.txt", &patterns).unwrap());
        assert!(config.matches_any_pattern("main.rs", &patterns).unwrap());
        assert!(!config.matches_any_pattern("other.txt", &patterns).unwrap());
    }

    #[test]
    fn test_is_allowed_file_read() {
        let config = PermissionConfig::new();
        assert!(
            config
                .is_allowed(PermissionType::FileRead, "any_file.txt")
                .unwrap()
        );
    }

    #[test]
    fn test_is_allowed_command_blocked() {
        let config = PermissionConfig::new();
        assert!(
            !config
                .is_allowed(PermissionType::CommandExecute, "rm -rf /")
                .unwrap()
        );
    }
}
