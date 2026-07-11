// This file contains tests generated with AI assistance.

use std::fs;
use std::path::PathBuf;

use rig_core::tool::Tool;
use tempfile::TempDir;

use super::*;

#[tokio::test]
async fn test_write_new_file() {
    let tmp = TempDir::new().unwrap();
    let file_path = tmp.path().join("hello.txt");

    let tool = WriteTool::new();
    let result = tool
        .call(WriteArgs { file_path: file_path.clone(), content: "Hello, world!".to_string() })
        .await
        .unwrap();

    assert!(result.success);
    assert_eq!(fs::read_to_string(&file_path).unwrap(), "Hello, world!");
}

#[tokio::test]
async fn test_write_creates_parent_dirs() {
    let tmp = TempDir::new().unwrap();

    let file_path = tmp.path().join("a/b/c/output.txt");
    let tool = WriteTool::new();
    let result = tool
        .call(WriteArgs { file_path: file_path.clone(), content: "nested content".to_string() })
        .await
        .unwrap();

    assert!(result.success);
    assert_eq!(fs::read_to_string(&file_path).unwrap(), "nested content");
}

#[tokio::test]
async fn test_write_overwrites_existing_file() {
    let tmp = TempDir::new().unwrap();

    let file_path = tmp.path().join("data.txt");
    fs::write(&file_path, "original").unwrap();

    let tool = WriteTool::new();
    let result = tool
        .call(WriteArgs { file_path: file_path.clone(), content: "overwritten".to_string() })
        .await
        .unwrap();

    assert!(result.success);
    assert_eq!(fs::read_to_string(&file_path).unwrap(), "overwritten");
}

#[tokio::test]
async fn test_write_empty_content() {
    let tmp = TempDir::new().unwrap();

    let file_path = tmp.path().join("empty.txt");
    let tool = WriteTool::new();
    let result = tool
        .call(WriteArgs { file_path: file_path.clone(), content: String::new() })
        .await
        .unwrap();

    assert!(result.success);
    assert_eq!(fs::read_to_string(&file_path).unwrap(), "");
}

#[test]
fn test_write_args_location_and_title() {
    let args = WriteArgs { file_path: PathBuf::from("/tmp/test.txt"), content: "blah".to_string() };

    let location: PathBuf = args.location().into();
    assert_eq!(location, PathBuf::from("/tmp/test.txt"));
    assert_eq!(args.title(), "Write file /tmp/test.txt");
}

#[tokio::test]
async fn test_write_tool_definition() {
    let tool = WriteTool::new();
    let def = tool.definition(String::new()).await;

    assert_eq!(def.name, "Write");
    assert!(!def.description.is_empty());
    let params = def.parameters;
    assert_eq!(params["type"], "object");

    let required = params["required"].as_array().unwrap();
    assert!(required.iter().any(|v| v.as_str() == Some("file_path")));
    assert!(required.iter().any(|v| v.as_str() == Some("content")));
}
