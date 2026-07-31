// This file contains tests generated with AI assistance.

use std::path::PathBuf;

use super::*;

#[test]
fn test_ls_args_title() {
    let args = LsArgs { path: Some(PathBuf::from("/tmp")), ignore: None };
    assert_eq!(args.title(), "List the /tmp directory's contents");

    let args = LsArgs { path: None, ignore: None };
    assert_eq!(args.title(), "List the . directory's contents");
}

#[tokio::test]
async fn test_ls_lists_directory() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut context = ToolContext::new();
    let tool = LsTool::new(tmp.path().to_path_buf());
    let result = tool.call(&mut context, LsArgs { path: None, ignore: None }).await.unwrap();
    assert!(result.entries.is_empty());

    // Create a file and verify it appears
    tokio::fs::write(tmp.path().join("hello.txt"), "content").await.unwrap();
    let result = tool.call(&mut context, LsArgs { path: None, ignore: None }).await.unwrap();
    assert!(result.entries.iter().any(|e| e == "hello.txt"));
}

#[tokio::test]
async fn test_ls_filters_by_ignore() {
    let tmp = tempfile::TempDir::new().unwrap();
    tokio::fs::write(tmp.path().join("a.txt"), "").await.unwrap();
    tokio::fs::write(tmp.path().join("b.log"), "").await.unwrap();

    let mut context = ToolContext::new();
    let tool = LsTool::new(tmp.path().to_path_buf());
    let result = tool
        .call(&mut context, LsArgs { path: None, ignore: Some(vec!["*.log".to_string()]) })
        .await
        .unwrap();
    assert!(result.entries.iter().any(|e| e == "a.txt"));
    assert!(!result.entries.iter().any(|e| e == "b.log"));
}
