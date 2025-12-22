#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolType {
    ReadFile,
    WriteFile,
    UpdateFile,
    ListDir,
    Glob,
    Grep,
    Bash,
    Custom(String),
}

impl ToolType {
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::ReadFile => "read_file",
            Self::WriteFile => "write_file",
            Self::UpdateFile => "update_file",
            Self::ListDir => "list_dir",
            Self::Glob => "glob",
            Self::Grep => "grep",
            Self::Bash => "bash",
            Self::Custom(name) => name,
        }
    }

    #[must_use]
    pub fn from_name(name: &str) -> Self {
        match name {
            "read_file" => Self::ReadFile,
            "write_file" => Self::WriteFile,
            "update_file" => Self::UpdateFile,
            "list_dir" => Self::ListDir,
            "glob" => Self::Glob,
            "grep" => Self::Grep,
            "bash" => Self::Bash,
            other => Self::Custom(other.to_string()),
        }
    }

    #[must_use]
    pub const fn is_read_only(&self) -> bool {
        matches!(
            self,
            Self::ReadFile | Self::ListDir | Self::Glob | Self::Grep
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolState {
    Starting,
    InProgress,
    Success,
    Error,
}
