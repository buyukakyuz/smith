use parking_lot::RwLock;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::config::PermissionConfig;
use super::security::SecurityValidator;
use super::types::{PermissionCheckResult, PermissionRequest, PermissionResponse, PermissionType};
use super::ui_trait::PermissionUI;
#[cfg(test)]
use super::ui_trait::test_utils::HeadlessPermissionUI;
use crate::core::error::Result;

#[derive(Debug, Default)]
struct SessionPermissions {
    allowed_permission_types: HashSet<PermissionType>,
}

impl SessionPermissions {
    fn is_allowed(&self, perm_type: PermissionType, _target: &str) -> bool {
        match perm_type {
            PermissionType::FileRead => true,
            _ => self.allowed_permission_types.contains(&perm_type),
        }
    }

    fn add_permission(&mut self, perm_type: PermissionType) {
        self.allowed_permission_types.insert(perm_type);
    }
}

pub struct PermissionManager {
    config: Arc<RwLock<PermissionConfig>>,
    session: Arc<RwLock<SessionPermissions>>,
    validator: SecurityValidator,
    ui: Arc<dyn PermissionUI>,
}

impl PermissionManager {
    pub fn new(ui: Arc<dyn PermissionUI>) -> Result<Self> {
        let config_path = PermissionConfig::default_permissions_file();
        Self::with_config_path(config_path, ui)
    }

    pub fn with_config_path(config_path: PathBuf, ui: Arc<dyn PermissionUI>) -> Result<Self> {
        let config = if config_path.exists() {
            PermissionConfig::load(&config_path)?
        } else {
            PermissionConfig::new()
        };

        let validator = SecurityValidator::new()?;

        Ok(Self {
            config: Arc::new(RwLock::new(config)),
            session: Arc::new(RwLock::new(SessionPermissions::default())),
            validator,
            ui,
        })
    }

    pub fn check_permission(&self, request: &PermissionRequest) -> Result<PermissionCheckResult> {
        self.validate_request(request)?;

        {
            let config = self.config.read();
            if config.is_allowed(request.operation_type, &request.target)? {
                return Ok(PermissionCheckResult::Allowed);
            }
        }

        {
            let session = self.session.read();
            if session.is_allowed(request.operation_type, &request.target) {
                return Ok(PermissionCheckResult::Allowed);
            }
        }

        let response = self.ui.prompt_user(request)?;

        match response {
            PermissionResponse::AllowOnce => Ok(PermissionCheckResult::Allowed),
            PermissionResponse::AllowSession => {
                self.add_session_permission(request);
                Ok(PermissionCheckResult::Allowed)
            }
            PermissionResponse::TellModelDifferently(feedback) => {
                Ok(PermissionCheckResult::DeniedWithFeedback(feedback))
            }
        }
    }

    fn validate_request(&self, request: &PermissionRequest) -> Result<()> {
        match request.operation_type {
            PermissionType::FileWrite => {
                self.validator
                    .validate_write_path(Path::new(&request.target))?;
            }
            PermissionType::FileDelete => {
                self.validator
                    .validate_delete_path(Path::new(&request.target))?;
            }
            _ => {}
        }
        Ok(())
    }

    fn add_session_permission(&self, request: &PermissionRequest) {
        self.session.write().add_permission(request.operation_type);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_manager() -> (PermissionManager, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("permissions.json");
        let manager = PermissionManager::with_config_path(
            config_path,
            Arc::new(HeadlessPermissionUI::deny()),
        )
        .unwrap();
        (manager, temp_dir)
    }

    #[tokio::test]
    async fn test_check_permission_file_read_allowed() {
        let (manager, _temp) = create_test_manager();
        let request = PermissionRequest::new(PermissionType::FileRead, "test.txt");
        let result = manager.check_permission(&request).unwrap();
        assert!(matches!(result, PermissionCheckResult::Allowed));
    }

    #[tokio::test]
    async fn test_check_permission_denied_without_prompt() {
        let (manager, _temp) = create_test_manager();
        let request = PermissionRequest::new(PermissionType::FileWrite, "test.txt");
        let result = manager.check_permission(&request).unwrap();
        assert!(matches!(
            result,
            PermissionCheckResult::DeniedWithFeedback(_)
        ));
    }

    #[tokio::test]
    async fn test_add_session_permission() {
        let (manager, _temp) = create_test_manager();
        let request = PermissionRequest::new(PermissionType::CommandExecute, "cargo build");

        manager.add_session_permission(&request);

        let session = manager.session.read();
        assert!(session.is_allowed(PermissionType::CommandExecute, "cargo build"));
        assert!(session.is_allowed(PermissionType::CommandExecute, "npm install"));
        assert!(session.is_allowed(PermissionType::CommandExecute, "any command"));
    }
}
