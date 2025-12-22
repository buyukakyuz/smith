use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ToolContext {
    pub working_dir: PathBuf,
    pub max_output_size: usize,
    pub default_timeout_ms: u64,
}

impl ToolContext {
    pub fn new() -> crate::core::error::Result<Self> {
        let working_dir = std::env::current_dir().map_err(crate::core::error::AgentError::Io)?;

        Ok(Self {
            working_dir,
            max_output_size: 10 * 1024 * 1024,
            default_timeout_ms: 120_000,
        })
    }

    #[must_use]
    pub const fn with_working_dir(working_dir: PathBuf) -> Self {
        Self {
            working_dir,
            max_output_size: 10 * 1024 * 1024,
            default_timeout_ms: 120_000,
        }
    }

    #[must_use]
    pub fn truncate_output(&self, output: String) -> (String, bool) {
        if output.len() > self.max_output_size {
            let truncated = output
                .chars()
                .take(self.max_output_size)
                .collect::<String>();
            let message = format!(
                "\n\n[Output truncated: {} bytes total, showing first {} bytes]",
                output.len(),
                self.max_output_size
            );
            (format!("{truncated}{message}"), true)
        } else {
            (output, false)
        }
    }
}

impl Default for ToolContext {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self::with_working_dir(PathBuf::from(".")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_creation() {
        let ctx = ToolContext::new().unwrap();
        assert_eq!(ctx.max_output_size, 10 * 1024 * 1024);
        assert_eq!(ctx.default_timeout_ms, 120_000);
    }

    #[test]
    fn test_context_with_working_dir() {
        let ctx = ToolContext::with_working_dir(PathBuf::from("/tmp"));
        assert_eq!(ctx.working_dir, PathBuf::from("/tmp"));
    }

    #[test]
    fn test_truncate_output() {
        let mut ctx = ToolContext::with_working_dir(PathBuf::from("."));
        ctx.max_output_size = 10;

        let (result, truncated) = ctx.truncate_output("short".to_string());
        assert_eq!(result, "short");
        assert!(!truncated);

        let (result, truncated) = ctx.truncate_output("this is a very long string".to_string());
        assert!(truncated);
        assert!(result.contains("truncated"));
        assert!(result.len() > 10);
    }
}
