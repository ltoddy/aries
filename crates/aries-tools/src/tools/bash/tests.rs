// This file contains tests generated with AI assistance.

use rig_core::tool::Tool;

use super::*;

#[tokio::test]
async fn test_bash_echo() {
    let tool = BashTool;
    let result = tool.call(BashArgs { command: "echo hello".to_owned() }).await.unwrap();

    assert_eq!(result.stdout.trim(), "hello");
    assert_eq!(result.exit_code, 0);
}

#[tokio::test]
async fn test_bash_failed_command() {
    let tool = BashTool;
    let result = tool.call(BashArgs { command: "exit 1".to_owned() }).await.unwrap();

    assert_eq!(result.exit_code, 1);
}

#[tokio::test]
async fn test_bash_nonexistent_command() {
    let tool = BashTool;
    let result = tool.call(BashArgs { command: "nonexistent_cmd_12345".to_owned() }).await.unwrap();

    assert_ne!(result.exit_code, 0);
    assert!(!result.stderr.is_empty());
}

#[tokio::test]
async fn test_bash_args_title() {
    let args = BashArgs { command: "echo test".to_owned() };
    assert_eq!(args.title(), "Run shell command: echo test");

    let empty_args = BashArgs { command: String::new() };
    assert_eq!(empty_args.title(), "Run a shell command");
}
