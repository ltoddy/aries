// This file contains tests generated with AI assistance.

use std::fs;

use rig_core::tool::Tool;
use tempfile::TempDir;

use super::*;

#[tokio::test]
async fn test_read_file() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("test.txt");
    fs::write(&file_path, "line1\nline2\nline3").unwrap();

    let tool = ReadTool::new();
    let result = tool.call(ReadArgs { file_path, offset: None }).await.unwrap();

    assert_eq!(result.content, "line1\nline2\nline3");
}

#[tokio::test]
async fn test_read_file_with_offset() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("test.txt");
    fs::write(&file_path, "line1\nline2\nline3").unwrap();

    let tool = ReadTool::new();
    let result = tool.call(ReadArgs { file_path, offset: Some(2) }).await.unwrap();

    assert_eq!(result.content, "line2\nline3");
}

#[tokio::test]
async fn test_read_file_not_found() {
    let tool = ReadTool::new();
    let result =
        tool.call(ReadArgs { file_path: "/nonexistent/file.txt".into(), offset: None }).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_read_args_title() {
    let args = ReadArgs { file_path: "/path/to/file.rs".into(), offset: None };
    assert!(args.title().contains("file.rs"));
}
