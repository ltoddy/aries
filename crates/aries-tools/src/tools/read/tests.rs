// This file contains tests generated with AI assistance.

use std::fs;

use rig::tool::Tool;
use tempfile::TempDir;

use super::inspector::{ContentType, MAX_SCAN_SIZE, inspect};
use super::*;
use crate::context::ToolContext;

#[tokio::test]
async fn test_read_file() {
    let dir = TempDir::new().expect("test operation should succeed");
    let file_path = dir.path().join("test.txt");
    fs::write(&file_path, "line1\nline2\nline3").expect("test operation should succeed");

    let mut context = rig::tool::ToolContext::new();
    let tool = ReadTool::new(
        dir.path(),
        ToolContext::new(None, {
            let (notifier, _) = aries_event::Notifier::channel();
            notifier
        }),
    );
    let result = tool
        .call(&mut context, ReadArgs { file_path, offset: None, limit: None })
        .await
        .expect("test operation should succeed");

    // 带行号输出，行号右对齐到 6 列 + U+2192 分隔。
    assert_eq!(result.content, "     1\u{2192}line1\n     2\u{2192}line2\n     3\u{2192}line3");
}

#[tokio::test]
async fn test_read_file_with_offset() {
    let dir = TempDir::new().expect("test operation should succeed");
    let file_path = dir.path().join("test.txt");
    fs::write(&file_path, "line1\nline2\nline3").expect("test operation should succeed");

    let mut context = rig::tool::ToolContext::new();
    let tool = ReadTool::new(
        dir.path(),
        ToolContext::new(None, {
            let (notifier, _) = aries_event::Notifier::channel();
            notifier
        }),
    );
    let result = tool
        .call(&mut context, ReadArgs { file_path, offset: Some(2), limit: None })
        .await
        .expect("test operation should succeed");

    // 从第 2 行开始，行号也从 2 起算。
    assert_eq!(result.content, "     2\u{2192}line2\n     3\u{2192}line3");
}

#[tokio::test]
async fn test_read_file_with_limit() {
    let dir = TempDir::new().expect("test operation should succeed");
    let file_path = dir.path().join("test.txt");
    fs::write(&file_path, "line1\nline2\nline3\nline4").expect("test operation should succeed");

    let mut context = rig::tool::ToolContext::new();
    let tool = ReadTool::new(
        dir.path(),
        ToolContext::new(None, {
            let (notifier, _) = aries_event::Notifier::channel();
            notifier
        }),
    );
    let result = tool
        .call(&mut context, ReadArgs { file_path, offset: Some(2), limit: Some(2) })
        .await
        .expect("test operation should succeed");

    // 从第 2 行起读 2 行：line2、line3。
    assert_eq!(result.content, "     2\u{2192}line2\n     3\u{2192}line3");
}

#[tokio::test]
async fn test_read_file_respects_default_line_cap() {
    let dir = TempDir::new().expect("test operation should succeed");
    let file_path = dir.path().join("big.txt");
    let content = (1..=2500).map(|i| format!("line{i}")).collect::<Vec<_>>().join("\n");
    fs::write(&file_path, content).expect("test operation should succeed");

    let mut context = rig::tool::ToolContext::new();
    let tool = ReadTool::new(
        dir.path(),
        ToolContext::new(None, {
            let (notifier, _) = aries_event::Notifier::channel();
            notifier
        }),
    );
    let result = tool
        .call(&mut context, ReadArgs { file_path, offset: None, limit: None })
        .await
        .expect("test operation should succeed");

    // 默认最多 2000 行，且标记为已截断。
    assert_eq!(result.content.lines().count(), MAX_LINES_TO_READ);
    assert!(result.content.starts_with("     1\u{2192}line1"));
    assert!(result.content.ends_with("line2000"));
    assert!(result.truncated);
}

#[tokio::test]
async fn test_read_file_rejects_zero_limit() {
    let dir = TempDir::new().expect("test operation should succeed");
    let file_path = dir.path().join("test.txt");
    fs::write(&file_path, "line1").expect("test operation should succeed");

    let mut context = rig::tool::ToolContext::new();
    let tool = ReadTool::new(
        dir.path(),
        ToolContext::new(None, {
            let (notifier, _) = aries_event::Notifier::channel();
            notifier
        }),
    );
    let result =
        tool.call(&mut context, ReadArgs { file_path, offset: None, limit: Some(0) }).await;

    assert!(matches!(result, Err(ReadError::InvalidLimit)));
}

#[tokio::test]
async fn test_read_empty_file() {
    let dir = TempDir::new().expect("test operation should succeed");
    let file_path = dir.path().join("empty.txt");
    fs::write(&file_path, "").expect("test operation should succeed");

    let mut context = rig::tool::ToolContext::new();
    let tool = ReadTool::new(
        dir.path(),
        ToolContext::new(None, {
            let (notifier, _) = aries_event::Notifier::channel();
            notifier
        }),
    );
    let result = tool
        .call(&mut context, ReadArgs { file_path, offset: None, limit: None })
        .await
        .expect("test operation should succeed");

    assert_eq!(result.content, EMPTY_FILE_NOTICE);
}

#[tokio::test]
async fn test_read_directory_is_rejected() {
    let dir = TempDir::new().expect("test operation should succeed");

    let mut context = rig::tool::ToolContext::new();
    let tool = ReadTool::new(
        dir.path(),
        ToolContext::new(None, {
            let (notifier, _) = aries_event::Notifier::channel();
            notifier
        }),
    );
    let result = tool
        .call(
            &mut context,
            ReadArgs { file_path: dir.path().to_path_buf(), offset: None, limit: None },
        )
        .await;

    assert!(matches!(result, Err(ReadError::IsADirectory(_))));
}

#[tokio::test]
async fn test_read_file_not_found() {
    let mut context = rig::tool::ToolContext::new();
    let tool = ReadTool::new(
        ".",
        ToolContext::new(None, {
            let (notifier, _) = aries_event::Notifier::channel();
            notifier
        }),
    );
    let result = tool
        .call(
            &mut context,
            ReadArgs { file_path: "/nonexistent/file.txt".into(), offset: None, limit: None },
        )
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_read_args_title() {
    let args = ReadArgs { file_path: "/path/to/file.rs".into(), offset: None, limit: None };
    assert!(args.title().contains("file.rs"));
}

#[tokio::test]
async fn test_read_file_marks_truncation() {
    let dir = TempDir::new().expect("test operation should succeed");
    let file_path = dir.path().join("test.txt");
    fs::write(&file_path, "line1\nline2\nline3\nline4").expect("test operation should succeed");

    let mut context = rig::tool::ToolContext::new();
    let tool = ReadTool::new(
        dir.path(),
        ToolContext::new(None, {
            let (notifier, _) = aries_event::Notifier::channel();
            notifier
        }),
    );
    let result = tool
        .call(&mut context, ReadArgs { file_path, offset: None, limit: Some(3) })
        .await
        .expect("test operation should succeed");

    assert_eq!(result.content, "     1\u{2192}line1\n     2\u{2192}line2\n     3\u{2192}line3");
    assert!(result.truncated);
}

#[tokio::test]
async fn test_read_file_not_truncated_at_exact_limit() {
    let dir = TempDir::new().expect("test operation should succeed");
    let file_path = dir.path().join("test.txt");
    fs::write(&file_path, "line1\nline2\nline3").expect("test operation should succeed");

    let mut context = rig::tool::ToolContext::new();
    let tool = ReadTool::new(
        dir.path(),
        ToolContext::new(None, {
            let (notifier, _) = aries_event::Notifier::channel();
            notifier
        }),
    );
    let result = tool
        .call(&mut context, ReadArgs { file_path, offset: None, limit: Some(3) })
        .await
        .expect("test operation should succeed");

    assert_eq!(result.content.lines().count(), 3);
    assert!(!result.truncated);
}

#[test]
fn test_render_output_appends_notice_only_when_truncated() {
    let truncated = ReadOutput::new("     1\u{2192}line1", true);
    let raw = serde_json::to_value(&truncated).expect("test operation should succeed");
    assert_eq!(
        ReadOutput::render_output(raw).expect("test operation should succeed"),
        "     1\u{2192}line1\n<system-reminder>File truncated; more lines follow. Use offset/limit to continue reading.</system-reminder>"
    );

    let complete = ReadOutput::new("     1\u{2192}line1", false);
    let raw = serde_json::to_value(&complete).expect("test operation should succeed");
    assert_eq!(
        ReadOutput::render_output(raw).expect("test operation should succeed"),
        "     1\u{2192}line1"
    );
}

#[tokio::test]
async fn test_read_file_rejects_binary() {
    let dir = TempDir::new().expect("test operation should succeed");
    let file_path = dir.path().join("test.bin");
    fs::write(&file_path, b"foo\x00bar").expect("test operation should succeed");

    let mut context = rig::tool::ToolContext::new();
    let tool = ReadTool::new(
        dir.path(),
        ToolContext::new(None, {
            let (notifier, _) = aries_event::Notifier::channel();
            notifier
        }),
    );
    let result = tool.call(&mut context, ReadArgs { file_path, offset: None, limit: None }).await;

    assert!(matches!(result, Err(ReadError::BinaryFile(_))));
}

#[test]
fn test_empty_buffer_is_utf8() {
    assert_eq!(inspect(b""), ContentType::Utf8);
}

#[test]
fn test_plain_text_is_utf8() {
    assert_eq!(inspect("Simple UTF-8 string ☔".as_bytes()), ContentType::Utf8);
}

#[test]
fn test_nul_byte_is_binary() {
    assert_eq!(inspect(b"foo\x00bar"), ContentType::Binary);
}

#[test]
fn test_nul_after_scan_window_is_utf8() {
    // 只扫描前 1024 字节，超出范围的 NUL 不会被发现
    let mut buffer = vec![b'a'; MAX_SCAN_SIZE];
    buffer.push(0x00);
    assert_eq!(inspect(&buffer), ContentType::Utf8);
}

#[test]
fn test_byte_order_marks() {
    assert_eq!(inspect(b"\xEF\xBB\xBFhello"), ContentType::Utf8Bom);
    assert_eq!(inspect(b"\xFF\xFEh\x00i\x00"), ContentType::Utf16Le);
    assert_eq!(inspect(b"\xFE\xFF\x00h\x00i"), ContentType::Utf16Be);
    assert_eq!(inspect(b"\xFF\xFE\x00\x00h\x00\x00\x00"), ContentType::Utf32Le);
    assert_eq!(inspect(b"\x00\x00\xFE\xFF\x00\x00\x00h"), ContentType::Utf32Be);
}

#[test]
fn test_magic_numbers_are_binary() {
    assert_eq!(inspect(b"%PDF-1.7"), ContentType::Binary);
    assert_eq!(inspect(b"\x89PNG\r\n\x1a\n"), ContentType::Binary);
}

#[test]
fn test_is_binary() {
    assert!(ContentType::Binary.is_binary());
    assert!(!ContentType::Utf8.is_binary());
    assert!(!ContentType::Utf32Le.is_binary());
}
