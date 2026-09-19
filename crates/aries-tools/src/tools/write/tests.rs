// This file contains tests generated with AI assistance.

use std::fs;
use std::path::PathBuf;

use rig::tool::Tool;
use tempfile::TempDir;

use super::*;
use crate::context::ToolContext;

#[tokio::test]
async fn test_write_empty_existing_file() {
    let tmp = TempDir::new().expect("test operation should succeed");
    let file_path = tmp.path().join("hello.txt");
    fs::write(&file_path, "").expect("test operation should succeed");

    let ctx = ToolContext::new(None, {
        let (notifier, _) = aries_event::Notifier::channel();
        notifier
    });
    ctx.on_file_read(&file_path).await;

    let mut context = rig::tool::ToolContext::new();
    let tool = WriteTool::new(tmp.path(), ctx);
    let result = tool
        .call(
            &mut context,
            WriteArgs { file_path: file_path.clone(), content: "Hello, world!".to_string() },
        )
        .await
        .expect("test operation should succeed");

    assert_eq!(result.file_path, file_path);
    assert_eq!(result.additions, 1);
    assert_eq!(
        fs::read_to_string(&file_path).expect("test operation should succeed"),
        "Hello, world!"
    );
}

#[tokio::test]
async fn test_write_new_file() {
    let tmp = TempDir::new().expect("test operation should succeed");
    let file_path = tmp.path().join("missing.txt");

    let mut context = rig::tool::ToolContext::new();
    let tool = WriteTool::new(
        tmp.path(),
        ToolContext::new(None, {
            let (notifier, _) = aries_event::Notifier::channel();
            notifier
        }),
    );
    let result = tool
        .call(
            &mut context,
            WriteArgs { file_path: file_path.clone(), content: "content".to_string() },
        )
        .await
        .expect("test operation should succeed");

    assert_eq!(result.file_path, file_path);
    assert_eq!(fs::read_to_string(&file_path).expect("test operation should succeed"), "content");
}

#[tokio::test]
async fn test_write_rejects_unread_empty_file() {
    let tmp = TempDir::new().expect("test operation should succeed");
    let file_path = tmp.path().join("empty.txt");
    fs::write(&file_path, "").expect("test operation should succeed");

    let ctx = ToolContext::new(None, {
        let (notifier, _) = aries_event::Notifier::channel();
        notifier
    });

    let mut context = rig::tool::ToolContext::new();
    let tool = WriteTool::new(tmp.path(), ctx);
    let result = tool
        .call(
            &mut context,
            WriteArgs { file_path: file_path.clone(), content: "content".to_string() },
        )
        .await;

    assert!(matches!(result, Err(WriteError::GuardWrite(_))));
    assert_eq!(fs::read_to_string(&file_path).expect("test operation should succeed"), "");
}

#[tokio::test]
async fn test_write_rejects_non_empty_existing_file() {
    let tmp = TempDir::new().expect("test operation should succeed");

    let file_path = tmp.path().join("data.txt");
    fs::write(&file_path, "line1\nline2\nline3\n").expect("test operation should succeed");

    let ctx = ToolContext::new(None, {
        let (notifier, _) = aries_event::Notifier::channel();
        notifier
    });

    let mut context = rig::tool::ToolContext::new();
    let tool = WriteTool::new(tmp.path(), ctx);
    let result = tool
        .call(
            &mut context,
            WriteArgs {
                file_path: file_path.clone(),
                content: "line1\nCHANGED\nline3\n".to_string(),
            },
        )
        .await;

    assert!(matches!(result, Err(WriteError::FileNotEmpty(path)) if path == file_path));
    assert_eq!(
        fs::read_to_string(&file_path).expect("test operation should succeed"),
        "line1\nline2\nline3\n"
    );
}

#[tokio::test]
async fn test_write_empty_content() {
    let tmp = TempDir::new().expect("test operation should succeed");

    let file_path = tmp.path().join("empty.txt");
    fs::write(&file_path, "").expect("test operation should succeed");

    let ctx = ToolContext::new(None, {
        let (notifier, _) = aries_event::Notifier::channel();
        notifier
    });
    ctx.on_file_read(&file_path).await;

    let mut context = rig::tool::ToolContext::new();
    let tool = WriteTool::new(tmp.path(), ctx);
    tool.call(&mut context, WriteArgs { file_path: file_path.clone(), content: String::new() })
        .await
        .expect("test operation should succeed");

    assert_eq!(fs::read_to_string(&file_path).expect("test operation should succeed"), "");
}

#[tokio::test]
async fn test_write_resolves_relative_path_against_cwd() {
    let tmp = TempDir::new().expect("test operation should succeed");

    let file_path = tmp.path().join("sub/rel.txt");

    let mut context = rig::tool::ToolContext::new();
    let tool = WriteTool::new(
        tmp.path(),
        ToolContext::new(None, {
            let (notifier, _) = aries_event::Notifier::channel();
            notifier
        }),
    );
    let result = tool
        .call(
            &mut context,
            WriteArgs { file_path: PathBuf::from("sub/rel.txt"), content: "relative".to_string() },
        )
        .await
        .expect("test operation should succeed");

    assert_eq!(result.file_path, file_path);
    assert_eq!(
        fs::read_to_string(tmp.path().join("sub/rel.txt")).expect("test operation should succeed"),
        "relative"
    );
}

#[test]
fn test_write_args_location_and_title() {
    let args = WriteArgs { file_path: PathBuf::from("/tmp/test.txt"), content: "blah".to_string() };

    let location: PathBuf = args.location().into();
    assert_eq!(location, PathBuf::from("/tmp/test.txt"));
    assert_eq!(args.title(), "Write file /tmp/test.txt");
}

#[test]
fn test_render_output_create() {
    let create = serde_json::to_value(&WriteOutput {
        file_path: PathBuf::from("/tmp/new.txt"),
        additions: 1,
    })
    .expect("test operation should succeed");
    assert_eq!(
        WriteOutput::render_output(create).expect("test operation should succeed"),
        "File created successfully at: /tmp/new.txt"
    );
}
