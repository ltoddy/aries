// This file contains tests generated with AI assistance.

use rig_core::tool::Tool;
use tempfile::TempDir;
use tokio::fs;

use super::*;

#[tokio::test]
async fn test_edit_simple_replacement() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("test.txt");
    fs::write(&file_path, "hello world").await.unwrap();

    let tool = EditTool;
    let result = tool
        .call(EditArgs {
            file_path: file_path.clone(),
            old_text: "hello".to_owned(),
            new_text: "hi".to_owned(),
            replace_all: false,
        })
        .await
        .unwrap();

    assert!(result.success);
    assert_eq!(fs::read_to_string(&file_path).await.unwrap(), "hi world");
}

#[tokio::test]
async fn test_edit_replace_all() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("test.txt");
    fs::write(&file_path, "foo foo foo").await.unwrap();

    let tool = EditTool;
    let result = tool
        .call(EditArgs {
            file_path: file_path.clone(),
            old_text: "foo".to_owned(),
            new_text: "bar".to_owned(),
            replace_all: true,
        })
        .await
        .unwrap();

    assert!(result.success);
    assert_eq!(fs::read_to_string(&file_path).await.unwrap(), "bar bar bar");
}

#[tokio::test]
async fn test_edit_file_not_found() {
    let tool = EditTool;
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
    fs::write(&file_path, "hello world").await.unwrap();

    let tool = EditTool;
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
    fs::write(&file_path, "a a a").await.unwrap();

    let tool = EditTool;
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
async fn test_edit_args_title() {
    let args = EditArgs {
        file_path: "/path/to/file.rs".into(),
        old_text: "old".to_owned(),
        new_text: "new".to_owned(),
        replace_all: false,
    };
    assert!(args.title().contains("file.rs"));
}
