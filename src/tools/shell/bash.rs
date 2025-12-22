use async_trait::async_trait;
use regex::Regex;
use schemars::JsonSchema;
use serde::Deserialize;
use std::collections::HashMap;
use std::fmt::Write;
use std::path::Path;
use std::process::Stdio;
use std::sync::LazyLock;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::time::timeout;

use crate::core::error::{AgentError, Result};
use crate::tools::{ToolType, TypedTool};

const DEFAULT_TIMEOUT_SECS: u64 = 120;
const MAX_OUTPUT_SIZE: usize = 1024 * 1024;

static ANSI_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\x1b\[[0-9;]*[a-zA-Z]").expect("valid regex"));

fn strip_ansi_codes(s: &str) -> String {
    ANSI_REGEX.replace_all(s, "").into_owned()
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BashInput {
    pub command: String,
    #[serde(default)]
    pub working_dir: Option<String>,
    #[serde(default)]
    pub timeout_secs: Option<u64>,
    #[serde(default)]
    pub env: Option<HashMap<String, String>>,
}

pub struct BashTool;

impl BashTool {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    async fn execute_command(
        command: &str,
        working_dir: Option<&Path>,
        timeout_secs: u64,
        env: Option<&HashMap<String, String>>,
    ) -> Result<String> {
        let wrapped_command = format!("( {command} ) 2>&1");

        let mut cmd = Command::new("sh");
        cmd.arg("-c")
            .arg(&wrapped_command)
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        if let Some(dir) = working_dir {
            cmd.current_dir(dir);
        }

        if let Some(env_vars) = env {
            for (key, value) in env_vars {
                cmd.env(key, value);
            }
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| AgentError::ToolExecution(format!("Failed to spawn command: {e}")))?;

        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| AgentError::ToolExecution("Failed to capture stdout".to_string()))?;

        let duration = Duration::from_secs(timeout_secs);
        let result = timeout(duration, async {
            let mut output_data = Vec::new();

            let read_result = stdout.read_to_end(&mut output_data).await;
            let status = child.wait().await?;

            read_result?;

            let output_str = if output_data.len() > MAX_OUTPUT_SIZE {
                let truncated = String::from_utf8_lossy(&output_data[..MAX_OUTPUT_SIZE]);
                format!("{truncated}\n\n[Output truncated: exceeded {MAX_OUTPUT_SIZE} bytes]")
            } else {
                String::from_utf8_lossy(&output_data).to_string()
            };

            Ok::<_, AgentError>((status, output_str))
        })
        .await;

        match result {
            Ok(Ok((status, output_str))) => {
                let exit_code = status.code().unwrap_or(-1);

                let clean_output = strip_ansi_codes(&output_str);

                let mut final_output = if clean_output.trim().is_empty() {
                    "[No output]".to_string()
                } else {
                    clean_output
                };

                let _ = write!(final_output, "\n\nExit code: {exit_code}");

                if exit_code != 0 {
                    return Err(AgentError::ToolExecution(final_output));
                }

                Ok(final_output)
            }
            Ok(Err(e)) => Err(e),
            Err(_) => {
                let _ = child.kill().await;
                Err(AgentError::Timeout(format!(
                    "Command timed out after {timeout_secs} seconds"
                )))
            }
        }
    }
}

impl Default for BashTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TypedTool for BashTool {
    type Input = BashInput;

    fn name(&self) -> &'static str {
        "bash"
    }

    fn description(&self) -> &'static str {
        "Execute a shell command. Returns stdout, stderr, and exit code. Commands are subject to timeout limits."
    }

    async fn execute_typed(&self, input: Self::Input) -> Result<String> {
        tracing::debug!("Executing bash command: {}", input.command);

        let working_dir = if let Some(dir_str) = &input.working_dir {
            let dir_path = Path::new(dir_str);

            if !dir_path.is_absolute() {
                tracing::warn!(
                    "Bash command rejected: working_dir not absolute: {}",
                    dir_str
                );
                return Err(AgentError::InvalidToolInput {
                    tool: ToolType::Bash.name().to_string(),
                    reason: format!("Working directory must be absolute: {dir_str}"),
                });
            }

            if !dir_path.exists() {
                tracing::warn!(
                    "Bash command rejected: working_dir does not exist: {}",
                    dir_str
                );
                return Err(AgentError::InvalidToolInput {
                    tool: ToolType::Bash.name().to_string(),
                    reason: format!("Working directory does not exist: {dir_str}"),
                });
            }

            if !dir_path.is_dir() {
                tracing::warn!(
                    "Bash command rejected: working_dir is not a directory: {}",
                    dir_str
                );
                return Err(AgentError::InvalidToolInput {
                    tool: ToolType::Bash.name().to_string(),
                    reason: format!("Working directory is not a directory: {dir_str}"),
                });
            }

            tracing::debug!("Using working directory: {}", dir_str);
            Some(dir_path)
        } else {
            None
        };

        let timeout_secs = input.timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS);
        tracing::debug!("Bash timeout: {}s", timeout_secs);

        let result = Self::execute_command(
            &input.command,
            working_dir,
            timeout_secs,
            input.env.as_ref(),
        )
        .await;

        match &result {
            Ok(output) => tracing::info!("Bash command succeeded: {} bytes output", output.len()),
            Err(e) => tracing::warn!("Bash command failed: {}", e),
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::Tool;

    #[test]
    fn test_bash_tool_name() {
        let tool = BashTool::new();
        assert_eq!(Tool::name(&tool), "bash");
    }

    #[tokio::test]
    async fn test_bash_tool_schema() {
        let tool = BashTool::new();
        let schema = tool.input_schema();
        assert!(schema.is_object());
    }

    #[tokio::test]
    async fn test_bash_simple_command() {
        let tool = BashTool::new();
        let input = BashInput {
            command: "echo 'Hello, World!'".to_string(),
            working_dir: None,
            timeout_secs: None,
            env: None,
        };

        let result = tool.execute_typed(input).await.unwrap();
        assert!(result.contains("Hello, World!"));
        assert!(result.contains("Exit code: 0"));
    }

    #[tokio::test]
    async fn test_bash_command_with_stderr() {
        let tool = BashTool::new();
        let input = BashInput {
            command: "echo 'stdout' && echo 'stderr' >&2".to_string(),
            working_dir: None,
            timeout_secs: None,
            env: None,
        };

        let result = tool.execute_typed(input).await.unwrap();
        assert!(result.contains("stdout"));
        assert!(result.contains("stderr"));
        assert!(result.contains("Exit code: 0"));
    }

    #[tokio::test]
    async fn test_bash_command_with_exit_code() {
        let tool = BashTool::new();
        let input = BashInput {
            command: "exit 42".to_string(),
            working_dir: None,
            timeout_secs: None,
            env: None,
        };

        let result = tool.execute_typed(input).await;
        assert!(result.is_err());

        if let Err(AgentError::ToolExecution(msg)) = result {
            assert!(msg.contains("Exit code: 42"));
        } else {
            panic!("Expected ToolExecution error");
        }
    }

    #[tokio::test]
    async fn test_bash_timeout() {
        let tool = BashTool::new();
        let input = BashInput {
            command: "sleep 10".to_string(),
            working_dir: None,
            timeout_secs: Some(1),
            env: None,
        };

        let result = tool.execute_typed(input).await;
        assert!(result.is_err());

        if let Err(AgentError::Timeout(msg)) = result {
            assert!(msg.contains("timed out"));
        } else {
            panic!("Expected timeout error");
        }
    }

    #[tokio::test]
    async fn test_bash_invalid_working_dir_relative() {
        let tool = BashTool::new();
        let input = BashInput {
            command: "pwd".to_string(),
            working_dir: Some("relative/path".to_string()),
            timeout_secs: None,
            env: None,
        };

        let result = tool.execute_typed(input).await;
        assert!(result.is_err());

        if let Err(AgentError::InvalidToolInput { reason, .. }) = result {
            assert!(reason.contains("must be absolute"));
        } else {
            panic!("Expected InvalidToolInput error");
        }
    }

    #[tokio::test]
    async fn test_bash_working_dir_not_exists() {
        let tool = BashTool::new();
        let input = BashInput {
            command: "pwd".to_string(),
            working_dir: Some("/nonexistent/directory/path".to_string()),
            timeout_secs: None,
            env: None,
        };

        let result = tool.execute_typed(input).await;
        assert!(result.is_err());

        if let Err(AgentError::InvalidToolInput { reason, .. }) = result {
            assert!(reason.contains("does not exist"));
        } else {
            panic!("Expected InvalidToolInput error");
        }
    }

    #[tokio::test]
    async fn test_bash_with_environment_variables() {
        let tool = BashTool::new();
        let mut env = HashMap::new();
        env.insert("TEST_VAR".to_string(), "test_value".to_string());
        env.insert("ANOTHER_VAR".to_string(), "another_value".to_string());

        let input = BashInput {
            command: "echo $TEST_VAR $ANOTHER_VAR".to_string(),
            working_dir: None,
            timeout_secs: None,
            env: Some(env),
        };

        let result = tool.execute_typed(input).await.unwrap();
        assert!(result.contains("test_value"));
        assert!(result.contains("another_value"));
    }

    #[tokio::test]
    async fn test_bash_strips_ansi_from_output() {
        let tool = BashTool::new();
        let input = BashInput {
            command: "printf '\\033[1m\\033[33mcolored\\033[0m text'".to_string(),
            working_dir: None,
            timeout_secs: None,
            env: None,
        };

        let result = tool.execute_typed(input).await.unwrap();
        assert!(result.contains("colored text"));
        assert!(!result.contains("\x1b["));
        assert!(!result.contains("[1m"));
        assert!(!result.contains("[33m"));
        assert!(!result.contains("[0m"));
    }

    #[tokio::test]
    async fn test_bash_strips_ansi_from_ls_color() {
        let tool = BashTool::new();
        let input = BashInput {
            command: "ls --color=always /tmp 2>/dev/null || ls -G /tmp".to_string(),
            working_dir: None,
            timeout_secs: None,
            env: None,
        };

        let result = tool.execute_typed(input).await.unwrap();
        assert!(
            !result.contains("\x1b["),
            "Output should not contain ANSI escape codes"
        );
    }

    #[tokio::test]
    async fn test_bash_strips_ansi_from_grep_color() {
        let tool = BashTool::new();
        let input = BashInput {
            command: "echo 'hello world' | grep --color=always hello".to_string(),
            working_dir: None,
            timeout_secs: None,
            env: None,
        };

        let result = tool.execute_typed(input).await.unwrap();
        assert!(result.contains("hello"));
        assert!(
            !result.contains("\x1b["),
            "Output should not contain ANSI escape codes"
        );
    }
}
