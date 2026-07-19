// This file contains tests generated with AI assistance.

use std::fs;

use rig_core::tool::Tool;
use tempfile::TempDir;

use super::*;
use crate::context::ToolContext;

#[tokio::test]
async fn test_read_file() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("test.txt");
    fs::write(&file_path, "line1\nline2\nline3").unwrap();

    let tool = ReadTool::new(dir.path(), ToolContext::new(None));
    let result = tool.call(ReadArgs { file_path, offset: None, limit: None }).await.unwrap();

    // 带行号输出，行号右对齐到 6 列 + U+2192 分隔。
    assert_eq!(result.content, "     1\u{2192}line1\n     2\u{2192}line2\n     3\u{2192}line3");
}

#[tokio::test]
async fn test_read_file_with_offset() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("test.txt");
    fs::write(&file_path, "line1\nline2\nline3").unwrap();

    let tool = ReadTool::new(dir.path(), ToolContext::new(None));
    let result = tool.call(ReadArgs { file_path, offset: Some(2), limit: None }).await.unwrap();

    // 从第 2 行开始，行号也从 2 起算。
    assert_eq!(result.content, "     2\u{2192}line2\n     3\u{2192}line3");
}

#[tokio::test]
async fn test_read_file_with_limit() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("test.txt");
    fs::write(&file_path, "line1\nline2\nline3\nline4").unwrap();

    let tool = ReadTool::new(dir.path(), ToolContext::new(None));
    let result = tool.call(ReadArgs { file_path, offset: Some(2), limit: Some(2) }).await.unwrap();

    // 从第 2 行起读 2 行：line2、line3。
    assert_eq!(result.content, "     2\u{2192}line2\n     3\u{2192}line3");
}

#[tokio::test]
async fn test_read_file_respects_default_line_cap() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("big.txt");
    let content = (1..=2500).map(|i| format!("line{i}")).collect::<Vec<_>>().join("\n");
    fs::write(&file_path, content).unwrap();

    let tool = ReadTool::new(dir.path(), ToolContext::new(None));
    let result = tool.call(ReadArgs { file_path, offset: None, limit: None }).await.unwrap();

    // 默认最多 2000 行。
    assert_eq!(result.content.lines().count(), MAX_LINES_TO_READ);
    assert!(result.content.starts_with("     1\u{2192}line1"));
    assert!(result.content.trim_end().ends_with("line2000"));
}

#[tokio::test]
async fn test_read_empty_file() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("empty.txt");
    fs::write(&file_path, "").unwrap();

    let tool = ReadTool::new(dir.path(), ToolContext::new(None));
    let result = tool.call(ReadArgs { file_path, offset: None, limit: None }).await.unwrap();

    assert_eq!(result.content, EMPTY_FILE_NOTICE);
}

#[tokio::test]
async fn test_read_directory_is_rejected() {
    let dir = TempDir::new().unwrap();

    let tool = ReadTool::new(dir.path(), ToolContext::new(None));
    let result = tool
        .call(ReadArgs { file_path: dir.path().to_path_buf(), offset: None, limit: None })
        .await;

    assert!(matches!(result, Err(ReadError::IsADirectory(_))));
}

#[tokio::test]
async fn test_read_file_not_found() {
    let tool = ReadTool::new(".", ToolContext::new(None));
    let result = tool
        .call(ReadArgs { file_path: "/nonexistent/file.txt".into(), offset: None, limit: None })
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_read_args_title() {
    let args = ReadArgs { file_path: "/path/to/file.rs".into(), offset: None, limit: None };
    assert!(args.title().contains("file.rs"));
}
