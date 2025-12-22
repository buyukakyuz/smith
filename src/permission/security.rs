use crate::core::error::{AgentError, Result};
use std::path::{Path, PathBuf};

pub struct SecurityValidator {
    working_dir: PathBuf,
    allow_outside_working_dir: bool,
}

impl SecurityValidator {
    pub fn new() -> Result<Self> {
        let working_dir = std::env::current_dir()?;
        let working_dir = working_dir.canonicalize().unwrap_or(working_dir);
        Ok(Self {
            working_dir,
            allow_outside_working_dir: false,
        })
    }

    pub fn validate_write_path(&self, path: &Path) -> Result<PathBuf> {
        let abs_path = self.resolve_path(path);

        if !self.allow_outside_working_dir && !self.is_within_working_dir(&abs_path) {
            return Err(AgentError::Config(format!(
                "Write access denied: path '{}' is outside working directory '{}'",
                abs_path.display(),
                self.working_dir.display()
            )));
        }

        if Self::is_system_directory(&abs_path) {
            return Err(AgentError::Config(format!(
                "Write access denied: '{}' is a system directory",
                abs_path.display()
            )));
        }

        Ok(abs_path)
    }

    pub fn validate_delete_path(&self, path: &Path) -> Result<PathBuf> {
        let abs_path = self.resolve_path(path);

        if !self.allow_outside_working_dir && !self.is_within_working_dir(&abs_path) {
            return Err(AgentError::Config(format!(
                "Delete access denied: path '{}' is outside working directory '{}'",
                abs_path.display(),
                self.working_dir.display()
            )));
        }

        if abs_path == self.working_dir {
            return Err(AgentError::Config(
                "Delete access denied: cannot delete working directory".to_string(),
            ));
        }

        if Self::is_system_directory(&abs_path) {
            return Err(AgentError::Config(format!(
                "Delete access denied: '{}' is a system directory",
                abs_path.display()
            )));
        }

        Ok(abs_path)
    }

    fn resolve_path(&self, path: &Path) -> PathBuf {
        let abs_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.working_dir.join(path)
        };

        abs_path.canonicalize().unwrap_or(abs_path)
    }

    fn is_within_working_dir(&self, path: &Path) -> bool {
        path.starts_with(&self.working_dir)
    }

    fn is_system_directory(path: &Path) -> bool {
        let path_str = path.to_string_lossy().to_lowercase();

        if path_str.starts_with("/bin")
            || path_str.starts_with("/sbin")
            || path_str.starts_with("/usr/bin")
            || path_str.starts_with("/usr/sbin")
            || path_str.starts_with("/etc")
            || path_str.starts_with("/sys")
            || path_str.starts_with("/proc")
            || path_str.starts_with("/dev")
            || path_str.starts_with("/boot")
            || path_str == "/"
        {
            return true;
        }

        if path_str.starts_with("c:\\windows")
            || path_str.starts_with("c:\\program files")
            || path_str.starts_with("c:\\system32")
        {
            return true;
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validator_creation() {
        let validator = SecurityValidator::new().unwrap();
        assert!(!validator.allow_outside_working_dir);
    }

    #[test]
    fn test_validate_write_path_system_directory() {
        let validator = SecurityValidator::new().unwrap();
        let system_path = PathBuf::from("/etc/passwd");

        let result = validator.validate_write_path(&system_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_is_system_directory() {
        assert!(SecurityValidator::is_system_directory(Path::new("/etc")));
        assert!(SecurityValidator::is_system_directory(Path::new("/bin")));
        assert!(!SecurityValidator::is_system_directory(Path::new(
            "/home/user"
        )));
    }
}
