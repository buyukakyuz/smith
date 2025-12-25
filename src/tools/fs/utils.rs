use std::path::{Path, PathBuf};

use ignore::WalkBuilder;

use crate::core::error::{AgentError, Result};
use crate::tools::types::ToolType;

use super::constants::MAX_FILE_SIZE;

pub fn is_absolute_path(path: &str) -> bool {
    Path::new(path).is_absolute()
}

pub fn validate_absolute_path(path: &str, tool_type: ToolType) -> Result<PathBuf> {
    if !is_absolute_path(path) {
        return Err(AgentError::InvalidToolInput {
            tool: tool_type.name().to_string(),
            reason: format!("Path must be absolute, got: {path}"),
        });
    }
    Ok(PathBuf::from(path))
}

pub fn validate_path_exists(path: &Path, tool_type: ToolType) -> Result<()> {
    if !path.exists() {
        return Err(AgentError::InvalidToolInput {
            tool: tool_type.name().to_string(),
            reason: format!("Path does not exist: {}", path.display()),
        });
    }
    Ok(())
}

pub fn validate_is_file(path: &Path, tool_type: ToolType) -> Result<()> {
    if !path.is_file() {
        return Err(AgentError::InvalidToolInput {
            tool: tool_type.name().to_string(),
            reason: format!("Path is not a file: {}", path.display()),
        });
    }
    Ok(())
}

pub fn validate_is_dir(path: &Path, tool_type: ToolType) -> Result<()> {
    if !path.is_dir() {
        return Err(AgentError::InvalidToolInput {
            tool: tool_type.name().to_string(),
            reason: format!("Path is not a directory: {}", path.display()),
        });
    }
    Ok(())
}

pub fn validate_file_size(path: &Path, tool_type: ToolType) -> Result<u64> {
    let metadata = std::fs::metadata(path)?;
    let size = metadata.len();
    if size > MAX_FILE_SIZE {
        return Err(AgentError::InvalidToolInput {
            tool: tool_type.name().to_string(),
            reason: format!("File too large: {size} bytes (max: {MAX_FILE_SIZE} bytes)"),
        });
    }
    Ok(size)
}

#[must_use]
pub fn walk_builder_with_gitignore(path: &Path, respect_gitignore: bool) -> WalkBuilder {
    let mut builder = WalkBuilder::new(path);
    builder
        .hidden(false)
        .parents(respect_gitignore)
        .git_ignore(respect_gitignore)
        .git_global(respect_gitignore)
        .git_exclude(respect_gitignore);
    builder
}
