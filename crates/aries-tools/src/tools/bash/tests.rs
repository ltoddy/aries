// This file contains tests generated with AI assistance.

use rig_agent::tool::{Tool, ToolContext};
use tempfile::TempDir;

use super::*;

fn bash_args(command: &str) -> BashArgs {
    BashArgs { command: command.to_owned(), timeout: None, description: None }
}

#[tokio::test]
async fn test_bash_echo() {
    let mut context = ToolContext::new();
    let tool = BashTool::new(std::env::temp_dir());
    let result = tool.call(&mut context, bash_args("echo hello")).await.unwrap();

    assert_eq!(result.stdout.trim(), "hello");
    assert_eq!(result.exit_code, 0);
}

#[tokio::test]
async fn test_bash_failed_command() {
    let mut context = ToolContext::new();
    let tool = BashTool::new(std::env::temp_dir());
    let result = tool.call(&mut context, bash_args("exit 1")).await.unwrap();

    assert_eq!(result.exit_code, 1);
}

#[tokio::test]
async fn test_bash_nonexistent_command() {
    let mut context = ToolContext::new();
    let tool = BashTool::new(std::env::temp_dir());
    let result = tool.call(&mut context, bash_args("nonexistent_cmd_12345")).await.unwrap();

    assert_ne!(result.exit_code, 0);
    assert!(!result.stderr.is_empty());
}

#[tokio::test]
async fn test_bash_runs_in_cwd() {
    let dir = TempDir::new().unwrap();
    let mut context = ToolContext::new();
    let tool = BashTool::new(dir.path());
    let result = tool.call(&mut context, bash_args("pwd")).await.unwrap();

    // 规范化后比较，规避 macOS 下 /var 与 /private/var 的软链接差异。
    let expected = std::fs::canonicalize(dir.path()).unwrap();
    let actual = std::fs::canonicalize(result.stdout.trim()).unwrap();
    assert_eq!(actual, expected);
}

#[tokio::test]
async fn test_bash_timeout() {
    let mut context = ToolContext::new();
    let tool = BashTool::new(std::env::temp_dir());
    let args = BashArgs { command: "sleep 5".to_owned(), timeout: Some(100), description: None };
    let result = tool.call(&mut context, args).await;

    assert!(matches!(result, Err(BashError::Timeout(100))));
}

#[tokio::test]
async fn test_bash_output_truncation() {
    let mut context = ToolContext::new();
    let tool = BashTool::new(std::env::temp_dir());
    // 打印约 40000 个字符，超过 30000 上限。
    let result = tool.call(&mut context, bash_args("printf 'a%.0s' $(seq 1 40000)")).await.unwrap();

    assert!(result.stdout.contains("lines truncated"));
    assert!(result.stdout.len() < 40_000);
}

#[tokio::test]
async fn test_bash_args_title() {
    let args = bash_args("echo test");
    assert_eq!(args.title(), "Run shell command: echo test");

    let empty_args = bash_args("");
    assert_eq!(empty_args.title(), "Run a shell command");
}

// --- tests for attempt_rewrite_last_command ---

#[test]
fn test_rewrite_single_command() {
    let tool = BashTool::new(std::env::temp_dir());
    assert_eq!(tool.attempt_rewrite_last_command("echo hello").unwrap(), "aries exec echo hello");
}

#[test]
fn test_rewrite_two_commands_with_and_and() {
    let tool = BashTool::new(std::env::temp_dir());
    assert_eq!(
        tool.attempt_rewrite_last_command("echo hello && ls -la").unwrap(),
        "echo hello && aries exec ls -la"
    );
}

#[test]
fn test_rewrite_two_commands_with_semicolon() {
    let tool = BashTool::new(std::env::temp_dir());
    assert_eq!(
        tool.attempt_rewrite_last_command("echo hello; echo world").unwrap(),
        "echo hello; aries exec echo world"
    );
}

#[test]
fn test_rewrite_two_commands_with_or_or() {
    let tool = BashTool::new(std::env::temp_dir());
    assert_eq!(
        tool.attempt_rewrite_last_command("false || echo fallback").unwrap(),
        "false || aries exec echo fallback"
    );
}

#[test]
fn test_rewrite_pipeline() {
    let tool = BashTool::new(std::env::temp_dir());
    // 管道中每个段是独立的 command 节点，最后一个段前插入 aries exec。
    assert_eq!(
        tool.attempt_rewrite_last_command("cat file | grep foo | wc -l").unwrap(),
        "cat file | grep foo | aries exec wc -l"
    );
}

#[test]
fn test_rewrite_newline_separated() {
    let tool = BashTool::new(std::env::temp_dir());
    assert_eq!(
        tool.attempt_rewrite_last_command("echo hello\necho world").unwrap(),
        "echo hello\naries exec echo world"
    );
}

#[test]
fn test_rewrite_three_commands() {
    let tool = BashTool::new(std::env::temp_dir());
    assert_eq!(
        tool.attempt_rewrite_last_command("echo a; echo b; echo c").unwrap(),
        "echo a; echo b; aries exec echo c"
    );
}

#[test]
fn test_rewrite_empty_returns_none() {
    let tool = BashTool::new(std::env::temp_dir());
    // 空字符串无法解析出 command 节点。
    assert!(tool.attempt_rewrite_last_command("").is_none());
}

#[test]
fn test_rewrite_with_comment() {
    let tool = BashTool::new(std::env::temp_dir());
    assert_eq!(
        tool.attempt_rewrite_last_command("echo hello # this is a comment").unwrap(),
        "aries exec echo hello # this is a comment"
    );
}
