// This file contains tests generated with AI assistance.

use std::fs;

use rig_core::tool::Tool;
use tempfile::TempDir;

use super::*;
use crate::context::ToolContext;

#[tokio::test]
async fn test_multiedit_basic() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("test.txt");
    fs::write(&file_path, "hello world").unwrap();

    let tool = MultiEditTool::new(dir.path(), ToolContext::new(None));
    let result = tool
        .call(MultiEditArgs {
            file_path: file_path.clone(),
            edits: vec![
                EditOperation {
                    old_text: "hello".to_owned(),
                    new_text: "hi".to_owned(),
                    replace_all: false,
                },
                EditOperation {
                    old_text: "world".to_owned(),
                    new_text: "earth".to_owned(),
                    replace_all: false,
                },
            ],
        })
        .await
        .unwrap();

    assert!(result.success);
    assert_eq!(fs::read_to_string(&file_path).unwrap(), "hi earth");
}

#[tokio::test]
async fn test_multiedit_creates_file() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("new_file.txt");

    let tool = MultiEditTool::new(dir.path(), ToolContext::new(None));
    let result = tool
        .call(MultiEditArgs {
            file_path: file_path.clone(),
            edits: vec![EditOperation {
                old_text: String::new(),
                new_text: "new content".to_owned(),
                replace_all: false,
            }],
        })
        .await
        .unwrap();

    assert!(result.success);
    assert_eq!(fs::read_to_string(&file_path).unwrap(), "new content");
}

#[tokio::test]
async fn test_multiedit_identical_text_error() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("test.txt");
    fs::write(&file_path, "hello").unwrap();

    let tool = MultiEditTool::new(dir.path(), ToolContext::new(None));
    let result = tool
        .call(MultiEditArgs {
            file_path,
            edits: vec![EditOperation {
                old_text: "hello".to_owned(),
                new_text: "hello".to_owned(),
                replace_all: false,
            }],
        })
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_multiedit_args_title() {
    let args = MultiEditArgs {
        file_path: "/path/to/file.rs".into(),
        edits: vec![EditOperation {
            old_text: "a".to_owned(),
            new_text: "b".to_owned(),
            replace_all: false,
        }],
    };
    assert!(args.title().contains("file.rs"));
    assert!(args.title().contains("1 changes"));
}
