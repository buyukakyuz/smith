use smith::tools::{GlobTool, GrepTool, ReadFileTool, Tool, WriteFileTool};
use tempfile::TempDir;

#[tokio::test]
async fn test_read_file_tool() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.txt");
    std::fs::write(&file_path, "Hello, World!\nLine 2\nLine 3").unwrap();

    let tool = ReadFileTool::new();
    let input = serde_json::json!({
        "path": file_path.to_string_lossy()
    });

    let result = tool.execute(input).await.unwrap();
    assert!(result.contains("Hello, World!"));
    assert!(result.contains("Line 2"));
}

#[tokio::test]
async fn test_read_file_tool_not_found() {
    let tool = ReadFileTool::new();
    let input = serde_json::json!({
        "path": "/nonexistent/path/file.txt"
    });

    let result = tool.execute(input).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_write_file_tool() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("output.txt");

    let tool = WriteFileTool::new();
    let input = serde_json::json!({
        "path": file_path.to_string_lossy(),
        "content": "Test content\nMultiple lines"
    });

    let result = tool.execute(input).await.unwrap();
    assert!(result.contains("Wrote"));
    assert!(result.contains("bytes"));

    let content = std::fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "Test content\nMultiple lines");
}

#[tokio::test]
async fn test_glob_tool() {
    let temp_dir = TempDir::new().unwrap();
    std::fs::write(temp_dir.path().join("file1.rs"), "").unwrap();
    std::fs::write(temp_dir.path().join("file2.rs"), "").unwrap();
    std::fs::write(temp_dir.path().join("other.txt"), "").unwrap();

    let tool = GlobTool::new();
    let input = serde_json::json!({
        "pattern": "*.rs",
        "base_dir": temp_dir.path().to_string_lossy(),
        "respect_gitignore": false
    });

    let result = tool.execute(input).await.unwrap();
    assert!(result.contains("file1.rs"));
    assert!(result.contains("file2.rs"));
    assert!(!result.contains("other.txt"));
}

#[tokio::test]
async fn test_grep_tool() {
    let temp_dir = TempDir::new().unwrap();
    std::fs::write(
        temp_dir.path().join("search.txt"),
        "Line with pattern\nAnother line\npattern again",
    )
    .unwrap();

    let tool = GrepTool::new();
    let input = serde_json::json!({
        "pattern": "pattern",
        "path": temp_dir.path().to_string_lossy()
    });

    let result = tool.execute(input).await.unwrap();
    assert!(result.contains("pattern"));
}

#[tokio::test]
async fn test_tool_schema_generation() {
    let read_tool = ReadFileTool::new();
    let schema = read_tool.input_schema();

    assert!(schema.is_object());
    let props = schema.get("properties");
    assert!(props.is_some());
}
