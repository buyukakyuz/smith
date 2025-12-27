use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use std::fmt::{self, Display};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::core::error::Result;
use crate::tools::ToolType;
use crate::tools::TypedTool;

use super::constants::{LIST_MAX_DEPTH, default_respect_gitignore};
use super::format::format_size;
use super::{
    validate_absolute_path, validate_is_dir, validate_path_exists, walk_builder_with_gitignore,
};

#[derive(Debug, Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SortBy {
    #[default]
    Name,
    Modified,
    Size,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListDirInput {
    pub path: String,
    #[serde(default)]
    pub include_hidden: bool,
    #[serde(default)]
    pub depth: usize,
    #[serde(default)]
    pub sort_by: SortBy,
    #[serde(default = "default_respect_gitignore")]
    #[schemars(default = "default_respect_gitignore")]
    pub respect_gitignore: bool,
}

#[derive(Debug)]
struct DirEntry {
    name: String,
    depth: usize,
    kind: EntryKind,
    modified: SystemTime,
}

#[derive(Debug)]
enum EntryKind {
    Directory,
    File { size: u64, executable: bool },
}

impl DirEntry {
    const fn is_dir(&self) -> bool {
        matches!(self.kind, EntryKind::Directory)
    }

    const fn size(&self) -> Option<u64> {
        match self.kind {
            EntryKind::File { size, .. } => Some(size),
            EntryKind::Directory => None,
        }
    }

    fn modified_timestamp(&self) -> u64 {
        self.modified
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs())
    }
}

impl DirEntry {
    fn from_walker_entry(entry: &ignore::DirEntry, base: &Path) -> Option<Self> {
        let path = entry.path();
        let metadata = entry.metadata().ok()?;

        let rel_path = path.strip_prefix(base).unwrap_or(path);
        let depth = rel_path.components().count().saturating_sub(1);

        let name = path.file_name()?.to_string_lossy().into_owned();

        let kind = if metadata.is_dir() {
            EntryKind::Directory
        } else {
            EntryKind::File {
                size: metadata.len(),
                executable: is_executable(&metadata),
            }
        };

        let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);

        Some(Self {
            name,
            depth,
            kind,
            modified,
        })
    }
}

#[cfg(unix)]
fn is_executable(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(_metadata: &std::fs::Metadata) -> bool {
    false
}
fn collect_entries(
    base: &Path,
    include_hidden: bool,
    depth: usize,
    respect_gitignore: bool,
) -> Vec<DirEntry> {
    walk_builder_with_gitignore(base, respect_gitignore)
        .hidden(!include_hidden)
        .max_depth(Some(depth + 1))
        .build()
        .flatten()
        .filter(|e| e.path() != base)
        .filter(|e| !is_inside_git_dir(e.path()))
        .filter_map(|e| DirEntry::from_walker_entry(&e, base))
        .collect()
}

fn is_inside_git_dir(path: &Path) -> bool {
    path.components().any(|c| c.as_os_str() == ".git")
}

fn sort_entries(entries: &mut [DirEntry], sort_by: SortBy) {
    match sort_by {
        SortBy::Name => entries.sort_by(|a, b| a.name.cmp(&b.name)),
        SortBy::Modified => {
            entries.sort_by_key(|e| std::cmp::Reverse(e.modified_timestamp()));
        }
        SortBy::Size => {
            entries.sort_by_key(|e| std::cmp::Reverse(e.size().unwrap_or(0)));
        }
    }
}

struct DirOutput<'a> {
    path: &'a Path,
    entries: &'a [DirEntry],
    sort_by: SortBy,
    depth: usize,
}

impl Display for DirOutput<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Directory: {}", self.path.display())?;

        if self.entries.is_empty() {
            return write!(f, "\nDirectory is empty");
        }

        let (files, dirs) = self.entries.iter().fold((0, 0), |(files, dirs), e| {
            if e.is_dir() {
                (files, dirs + 1)
            } else {
                (files + 1, dirs)
            }
        });

        writeln!(f, "Total: {files} files, {dirs} directories\n")?;

        for entry in self.entries {
            let indent = "  ".repeat(entry.depth);

            match &entry.kind {
                EntryKind::Directory => {
                    writeln!(f, "{indent}{}/", entry.name)?;
                }
                EntryKind::File { size, executable } => {
                    let suffix = if *executable { "*" } else { "" };
                    writeln!(f, "{indent}{}{suffix} ({})", entry.name, format_size(*size))?;
                }
            }
        }

        write!(f, "\n[Sorted by: {}]", self.sort_by)?;

        if self.depth > 0 {
            write!(f, "\n[Depth: {}]", self.depth)?;
        }

        Ok(())
    }
}

impl Display for SortBy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Name => write!(f, "name"),
            Self::Modified => write!(f, "modified"),
            Self::Size => write!(f, "size"),
        }
    }
}

#[derive(Default)]
pub struct ListDirTool;

impl ListDirTool {
    #[must_use]
    pub const fn new() -> Self {
        Self
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
        let path = validate_absolute_path(&input.path, &ToolType::ListDir)?;
        validate_path_exists(&path, &ToolType::ListDir)?;
        validate_is_dir(&path, &ToolType::ListDir)?;

        let depth = input.depth.min(LIST_MAX_DEPTH);

        let mut entries =
            collect_entries(&path, input.include_hidden, depth, input.respect_gitignore);
        sort_entries(&mut entries, input.sort_by);

        let output = DirOutput {
            path: &path,
            entries: &entries,
            sort_by: input.sort_by,
            depth,
        };

        Ok(output.to_string())
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
            sort_by: SortBy::Name,
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
            sort_by: SortBy::Name,
            respect_gitignore: false,
        };
        let result = tool.execute_typed(input).await.unwrap();
        assert!(result.contains("visible.txt"));
        assert!(!result.contains(".hidden"));

        let input = ListDirInput {
            path: dir_path.to_str().unwrap().to_string(),
            include_hidden: true,
            depth: 0,
            sort_by: SortBy::Name,
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
            sort_by: SortBy::Name,
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
            sort_by: SortBy::Name,
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
            sort_by: SortBy::Name,
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
            sort_by: SortBy::Name,
            respect_gitignore: true,
        };
        let result = tool.execute_typed(input).await.unwrap();
        assert!(result.contains("visible.txt"));
        assert!(!result.contains("ignored.txt"));

        let input = ListDirInput {
            path: dir_path.to_str().unwrap().to_string(),
            include_hidden: false,
            depth: 0,
            sort_by: SortBy::Name,
            respect_gitignore: false,
        };
        let result = tool.execute_typed(input).await.unwrap();
        assert!(result.contains("visible.txt"));
        assert!(result.contains("ignored.txt"));
    }
}
