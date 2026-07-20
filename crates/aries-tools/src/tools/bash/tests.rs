// This file contains tests generated with AI assistance.

use rig_core::tool::Tool;
use tempfile::TempDir;

use super::*;

fn bash_args(command: &str) -> BashArgs {
    BashArgs { command: command.to_owned(), timeout: None, description: None }
}

#[tokio::test]
async fn test_bash_echo() {
    let tool = BashTool::new(std::env::temp_dir());
    let result = tool.call(bash_args("echo hello")).await.unwrap();

    assert_eq!(result.stdout.trim(), "hello");
    assert_eq!(result.exit_code, 0);
}

#[tokio::test]
async fn test_bash_failed_command() {
    let tool = BashTool::new(std::env::temp_dir());
    let result = tool.call(bash_args("exit 1")).await.unwrap();

    assert_eq!(result.exit_code, 1);
}

#[tokio::test]
async fn test_bash_nonexistent_command() {
    let tool = BashTool::new(std::env::temp_dir());
    let result = tool.call(bash_args("nonexistent_cmd_12345")).await.unwrap();

    assert_ne!(result.exit_code, 0);
    assert!(!result.stderr.is_empty());
}

#[tokio::test]
async fn test_bash_runs_in_cwd() {
    let dir = TempDir::new().unwrap();
    let tool = BashTool::new(dir.path());
    let result = tool.call(bash_args("pwd")).await.unwrap();

    // 规范化后比较，规避 macOS 下 /var 与 /private/var 的软链接差异。
    let expected = std::fs::canonicalize(dir.path()).unwrap();
    let actual = std::fs::canonicalize(result.stdout.trim()).unwrap();
    assert_eq!(actual, expected);
}

#[tokio::test]
async fn test_bash_timeout() {
    let tool = BashTool::new(std::env::temp_dir());
    let args = BashArgs { command: "sleep 5".to_owned(), timeout: Some(100), description: None };
    let result = tool.call(args).await;

    assert!(matches!(result, Err(BashError::Timeout(100))));
}

#[tokio::test]
async fn test_bash_output_truncation() {
    let tool = BashTool::new(std::env::temp_dir());
    // 打印约 40000 个字符，超过 30000 上限。
    let result = tool.call(bash_args("printf 'a%.0s' $(seq 1 40000)")).await.unwrap();

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
