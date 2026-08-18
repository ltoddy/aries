// This file contains tests generated with AI assistance.

use std::fs;

use rig::tool::Tool;
use tempfile::TempDir;

use super::*;
use crate::context::ToolContext;
use crate::multiedit::WriteKind;

/// 写入文件并在共享 ctx 中登记一次完整读取，模拟“先 Read 后 MultiEdit”。
async fn seed_file(ctx: &ToolContext, path: &std::path::Path, content: &str) {
    fs::write(path, content).unwrap();
    ctx.on_file_read(path).await;
}

#[tokio::test]
async fn test_multiedit_basic() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("test.txt");
    let ctx = ToolContext::new(None);
    seed_file(&ctx, &file_path, "hello world").await;

    let mut context = rig::tool::ToolContext::new();
    let tool = MultiEditTool::new(dir.path(), ctx);
    let result = tool
        .call(
            &mut context,
            MultiEditArgs {
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
            },
        )
        .await
        .unwrap();

    assert_eq!(result.kind, WriteKind::Update);
    assert_eq!(result.original_content.as_deref(), Some("hello world"));
    assert!(!result.structured_patch.is_empty());
    assert_eq!(fs::read_to_string(&file_path).unwrap(), "hi earth");
}

#[tokio::test]
async fn test_multiedit_creates_file() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("new_file.txt");

    let mut context = rig::tool::ToolContext::new();
    let tool = MultiEditTool::new(dir.path(), ToolContext::new(None));
    let result = tool
        .call(
            &mut context,
            MultiEditArgs {
                file_path: file_path.clone(),
                edits: vec![EditOperation {
                    old_text: String::new(),
                    new_text: "new content".to_owned(),
                    replace_all: false,
                }],
            },
        )
        .await
        .unwrap();

    assert_eq!(result.kind, WriteKind::Create);
    assert!(result.original_content.is_none());
    assert_eq!(fs::read_to_string(&file_path).unwrap(), "new content");
}

#[tokio::test]
async fn test_multiedit_identical_text_error() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("test.txt");
    let ctx = ToolContext::new(None);
    seed_file(&ctx, &file_path, "hello").await;

    let mut context = rig::tool::ToolContext::new();
    let tool = MultiEditTool::new(dir.path(), ctx);
    let result = tool
        .call(
            &mut context,
            MultiEditArgs {
                file_path,
                edits: vec![EditOperation {
                    old_text: "hello".to_owned(),
                    new_text: "hello".to_owned(),
                    replace_all: false,
                }],
            },
        )
        .await;

    assert!(matches!(result, Err(MultiEditError::IdenticalText)));
}

#[tokio::test]
async fn test_multiedit_rejects_unread_file() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("test.txt");
    // 已存在但未经 Read：应被读后写校验拒绝。
    fs::write(&file_path, "hello world").unwrap();

    let mut context = rig::tool::ToolContext::new();
    let tool = MultiEditTool::new(dir.path(), ToolContext::new(None));
    let result = tool
        .call(
            &mut context,
            MultiEditArgs {
                file_path,
                edits: vec![EditOperation {
                    old_text: "hello".to_owned(),
                    new_text: "hi".to_owned(),
                    replace_all: false,
                }],
            },
        )
        .await;

    assert!(matches!(result, Err(MultiEditError::Guard(_))));
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
