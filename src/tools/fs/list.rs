use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::core::error::Result;
use crate::tools::TypedTool;

use super::{
    validate_absolute_path, validate_is_dir, validate_path_exists, walk_builder_with_gitignore,
};
use crate::tools::ToolType;

const MAX_DEPTH: usize = 5;

fn format_size(size: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if size >= GB {
        format!("{:.1} GB", size as f64 / GB as f64)
    } else if size >= MB {
        format!("{:.1} MB", size as f64 / MB as f64)
    } else if size >= KB {
        format!("{:.1} KB", size as f64 / KB as f64)
    } else {
        format!("{size} B")
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListDirInput {
    pub path: String,

    #[serde(default)]
    pub include_hidden: bool,

    #[serde(default)]
    pub depth: usize,

    #[serde(default = "default_sort")]
    pub sort_by: String,

    #[serde(default = "default_respect_gitignore")]
    #[schemars(default = "default_respect_gitignore")]
    pub respect_gitignore: bool,
}

fn default_sort() -> String {
    "name".to_string()
}

const fn default_respect_gitignore() -> bool {
    true
}

pub struct ListDirTool;

impl ListDirTool {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for ListDirTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TypedTool for ListDirTool {
    type Input = ListDirInput;

    fn name(&self) -> &'static str {
        "list_dir"
    }

    fn description(&self) -> &'static str {
        "List the contents of a directory with optional recursion and sorting. Shows file sizes and type indicators. Respects .gitignore by default. The path must be absolute."
    }

    async fn execute_typed(&self, input: Self::Input) -> Result<String> {
        let path = validate_absolute_path(&input.path, ToolType::ListDir)?;

        let depth = input.depth.min(MAX_DEPTH);

        validate_path_exists(&path, ToolType::ListDir)?;
        validate_is_dir(&path, ToolType::ListDir)?;

        let walker = walk_builder_with_gitignore(&path, input.respect_gitignore)
            .hidden(!input.include_hidden)
            .max_depth(Some(depth + 1))
            .build();

        let mut items: Vec<(String, Option<u64>, bool, u64)> = Vec::new();

        for entry in walker.flatten() {
            let entry_path = entry.path();

            if entry_path == path {
                continue;
            }

            if entry_path.components().any(|c| c.as_os_str() == ".git") {
                continue;
            }

            let rel_path = entry_path.strip_prefix(&path).unwrap_or(entry_path);
            let indent = rel_path.components().count().saturating_sub(1);

            let file_name = entry_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();

            let metadata = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };

            let is_dir = metadata.is_dir();
            let size = if is_dir { None } else { Some(metadata.len()) };
            let modified = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_secs());

            let indent_str = "  ".repeat(indent);
            let entry_str = if is_dir {
                format!("{indent_str}{file_name}/")
            } else {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let mode = metadata.permissions().mode();
                    if mode & 0o111 != 0 {
                        format!("{indent_str}{file_name}*")
                    } else {
                        format!("{indent_str}{file_name}")
                    }
                }
                #[cfg(not(unix))]
                {
                    format!("{indent_str}{file_name}")
                }
            };

            items.push((entry_str, size, is_dir, modified));
        }

        if items.is_empty() {
            return Ok(format!(
                "Directory: {}\n\nDirectory is empty",
                path.display()
            ));
        }

        match input.sort_by.as_str() {
            "modified" => items.sort_by(|a, b| b.3.cmp(&a.3)),
            "size" => items.sort_by(|a, b| {
                let size_a = a.1.unwrap_or(0);
                let size_b = b.1.unwrap_or(0);
                size_b.cmp(&size_a)
            }),
            _ => items.sort_by(|a, b| a.0.cmp(&b.0)),
        }

        let file_count = items.iter().filter(|(_, _, is_dir, _)| !is_dir).count();
        let dir_count = items.iter().filter(|(_, _, is_dir, _)| *is_dir).count();

        let mut output = String::new();
        output.push_str(&format!("Directory: {}\n", path.display()));
        output.push_str(&format!(
            "Total: {file_count} files, {dir_count} directories\n\n"
        ));

        for (name, size, _, _) in &items {
            if let Some(s) = size {
                output.push_str(&format!("{} ({})\n", name, format_size(*s)));
            } else {
                output.push_str(&format!("{name}\n"));
            }
        }

        output.push_str(&format!("\n[Sorted by: {}]", input.sort_by));
        if depth > 0 {
            output.push_str(&format!("\n[Depth: {depth}]"));
        }
        if input.respect_gitignore {
            output.push_str("\n[Respecting .gitignore]");
        }

        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::Tool;

    #[test]
    fn test_list_dir_tool_name() {
        let tool = ListDirTool::new();
        assert_eq!(Tool::name(&tool), "list_dir");
    }

    #[tokio::test]
    async fn test_list_dir_basic() {
        let temp_dir = tempfile::tempdir().unwrap();
        let dir_path = temp_dir.path();

        std::fs::File::create(dir_path.join("file1.txt")).unwrap();
        std::fs::File::create(dir_path.join("file2.txt")).unwrap();
        std::fs::create_dir(dir_path.join("subdir")).unwrap();

        let tool = ListDirTool::new();
        let input = ListDirInput {
            path: dir_path.to_str().unwrap().to_string(),
            include_hidden: false,
            depth: 0,
            sort_by: "name".to_string(),
            respect_gitignore: false,
        };

        let result = tool.execute_typed(input).await.unwrap();
        assert!(result.contains("file1.txt"));
        assert!(result.contains("file2.txt"));
        assert!(result.contains("subdir/"));
    }

    #[tokio::test]
    async fn test_list_dir_hidden_files() {
        let temp_dir = tempfile::tempdir().unwrap();
        let dir_path = temp_dir.path();

        std::fs::File::create(dir_path.join("visible.txt")).unwrap();
        std::fs::File::create(dir_path.join(".hidden")).unwrap();

        let tool = ListDirTool::new();

        let input = ListDirInput {
            path: dir_path.to_str().unwrap().to_string(),
            include_hidden: false,
            depth: 0,
            sort_by: "name".to_string(),
            respect_gitignore: false,
        };
        let result = tool.execute_typed(input).await.unwrap();
        assert!(result.contains("visible.txt"));
        assert!(!result.contains(".hidden"));

        let input = ListDirInput {
            path: dir_path.to_str().unwrap().to_string(),
            include_hidden: true,
            depth: 0,
            sort_by: "name".to_string(),
            respect_gitignore: false,
        };
        let result = tool.execute_typed(input).await.unwrap();
        assert!(result.contains("visible.txt"));
        assert!(result.contains(".hidden"));
    }

    #[tokio::test]
    async fn test_list_dir_recursive() {
        let temp_dir = tempfile::tempdir().unwrap();
        let dir_path = temp_dir.path();

        std::fs::File::create(dir_path.join("file.txt")).unwrap();
        std::fs::create_dir(dir_path.join("subdir")).unwrap();
        std::fs::File::create(dir_path.join("subdir").join("nested.txt")).unwrap();

        let tool = ListDirTool::new();
        let input = ListDirInput {
            path: dir_path.to_str().unwrap().to_string(),
            include_hidden: false,
            depth: 1,
            sort_by: "name".to_string(),
            respect_gitignore: false,
        };

        let result = tool.execute_typed(input).await.unwrap();
        assert!(result.contains("file.txt"));
        assert!(result.contains("subdir/"));
        assert!(result.contains("nested.txt"));
    }

    #[tokio::test]
    async fn test_list_dir_not_found() {
        let tool = ListDirTool::new();
        let input = ListDirInput {
            path: "/nonexistent/directory".to_string(),
            include_hidden: false,
            depth: 0,
            sort_by: "name".to_string(),
            respect_gitignore: false,
        };

        let result = tool.execute_typed(input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_dir_empty() {
        let temp_dir = tempfile::tempdir().unwrap();
        let dir_path = temp_dir.path();

        let tool = ListDirTool::new();
        let input = ListDirInput {
            path: dir_path.to_str().unwrap().to_string(),
            include_hidden: false,
            depth: 0,
            sort_by: "name".to_string(),
            respect_gitignore: false,
        };

        let result = tool.execute_typed(input).await.unwrap();
        assert!(result.contains("empty"));
    }

    #[tokio::test]
    async fn test_list_dir_respects_gitignore() {
        let temp_dir = tempfile::tempdir().unwrap();
        let dir_path = temp_dir.path();

        std::fs::create_dir(dir_path.join(".git")).unwrap();

        std::fs::write(dir_path.join(".gitignore"), "ignored.txt\n").unwrap();

        std::fs::File::create(dir_path.join("visible.txt")).unwrap();
        std::fs::File::create(dir_path.join("ignored.txt")).unwrap();

        let tool = ListDirTool::new();

        let input = ListDirInput {
            path: dir_path.to_str().unwrap().to_string(),
            include_hidden: false,
            depth: 0,
            sort_by: "name".to_string(),
            respect_gitignore: true,
        };
        let result = tool.execute_typed(input).await.unwrap();
        assert!(result.contains("visible.txt"));
        assert!(!result.contains("ignored.txt"));

        let input = ListDirInput {
            path: dir_path.to_str().unwrap().to_string(),
            include_hidden: false,
            depth: 0,
            sort_by: "name".to_string(),
            respect_gitignore: false,
        };
        let result = tool.execute_typed(input).await.unwrap();
        assert!(result.contains("visible.txt"));
        assert!(result.contains("ignored.txt"));
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(100), "100 B");
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1024 * 1024), "1.0 MB");
        assert_eq!(format_size(1024 * 1024 * 1024), "1.0 GB");
    }
}
