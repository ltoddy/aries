// This file contains tests generated with AI assistance.

use rig_core::tool::Tool;
use tempfile::TempDir;
use tokio::fs;

use super::*;
use crate::context::ToolContext;

/// 写入文件并在共享 ctx 中登记一次完整读取，模拟“先 Read 后 Edit”。
async fn seed_file(ctx: &ToolContext, path: &std::path::Path, content: &str) {
    fs::write(path, content).await.unwrap();
    ctx.on_file_read(path, false).await;
}

#[tokio::test]
async fn test_edit_simple_replacement() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("test.txt");
    let ctx = ToolContext::new(None);
    seed_file(&ctx, &file_path, "hello world").await;

    let tool = EditTool::new(dir.path(), ctx);
    let result = tool
        .call(EditArgs {
            file_path: file_path.clone(),
            old_text: "hello".to_owned(),
            new_text: "hi".to_owned(),
            replace_all: false,
        })
        .await
        .unwrap();

    assert_eq!(result.file_path, file_path);
    assert_eq!(result.original_content.as_deref(), Some("hello world"));
    assert!(!result.structured_patch.is_empty());
    assert_eq!((result.additions, result.deletions), (1, 1));
    assert_eq!(fs::read_to_string(&file_path).await.unwrap(), "hi world");
}

#[tokio::test]
async fn test_edit_replace_all() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("test.txt");
    let ctx = ToolContext::new(None);
    seed_file(&ctx, &file_path, "foo foo foo").await;

    let tool = EditTool::new(dir.path(), ctx);
    let result = tool
        .call(EditArgs {
            file_path: file_path.clone(),
            old_text: "foo".to_owned(),
            new_text: "bar".to_owned(),
            replace_all: true,
        })
        .await
        .unwrap();

    assert_eq!(result.file_path, file_path);
    assert_eq!(fs::read_to_string(&file_path).await.unwrap(), "bar bar bar");
}

#[tokio::test]
async fn test_edit_file_not_found() {
    let dir = TempDir::new().unwrap();
    let tool = EditTool::new(dir.path(), ToolContext::new(None));
    let result = tool
        .call(EditArgs {
            file_path: "/nonexistent/file.txt".into(),
            old_text: "hello".to_owned(),
            new_text: "hi".to_owned(),
            replace_all: false,
        })
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_edit_old_text_not_found() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("test.txt");
    let ctx = ToolContext::new(None);
    seed_file(&ctx, &file_path, "hello world").await;

    let tool = EditTool::new(dir.path(), ctx);
    let result = tool
        .call(EditArgs {
            file_path,
            old_text: "nonexistent".to_owned(),
            new_text: "hi".to_owned(),
            replace_all: false,
        })
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_edit_multiple_matches_without_replace_all() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("test.txt");
    let ctx = ToolContext::new(None);
    seed_file(&ctx, &file_path, "a a a").await;

    let tool = EditTool::new(dir.path(), ctx);
    let result = tool
        .call(EditArgs {
            file_path,
            old_text: "a".to_owned(),
            new_text: "b".to_owned(),
            replace_all: false,
        })
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_edit_rejects_unread_file() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("test.txt");
    // 未经 Read，直接编辑：应被读后写校验拒绝。
    fs::write(&file_path, "hello world").await.unwrap();

    let tool = EditTool::new(dir.path(), ToolContext::new(None));
    let result = tool
        .call(EditArgs {
            file_path,
            old_text: "hello".to_owned(),
            new_text: "hi".to_owned(),
            replace_all: false,
        })
        .await;

    assert!(matches!(result, Err(EditError::Guard(_))));
}

#[tokio::test]
async fn test_edit_rejects_modified_since_read() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("test.txt");
    let ctx = ToolContext::new(None);
    seed_file(&ctx, &file_path, "hello world").await;

    // 读取之后文件被外部修改（mtime 前移），编辑应被拒绝。
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    fs::write(&file_path, "hello brave world").await.unwrap();

    let tool = EditTool::new(dir.path(), ctx);
    let result = tool
        .call(EditArgs {
            file_path,
            old_text: "hello".to_owned(),
            new_text: "hi".to_owned(),
            replace_all: false,
        })
        .await;

    assert!(matches!(result, Err(EditError::Guard(_))));
}

#[tokio::test]
async fn test_edit_args_title() {
    let args = EditArgs {
        file_path: "/path/to/file.rs".into(),
        old_text: "old".to_owned(),
        new_text: "new".to_owned(),
        replace_all: false,
    };
    assert!(args.title().contains("file.rs"));
}
