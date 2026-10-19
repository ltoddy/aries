// This file contains tests generated with AI assistance.

use super::*;
use crate::hook::definition::BashCommandHook;
use crate::hook::executor::command::BashHookError;

#[tokio::test]
async fn bash_hook_exit_zero_does_not_block() {
    let hook: BashCommandHook = serde_json::from_str(r#"{"command": "echo hello"}"#)
        .expect("test operation should succeed");
    let outcome =
        execute_bash_command_hook(&hook, "").await.expect("test operation should succeed");
    assert_eq!(outcome.exit_code, Some(0));
    assert!(!outcome.blocked);
    assert!(outcome.stdout.contains("hello"));
}

#[tokio::test]
async fn bash_hook_exit_two_blocks() {
    let hook: BashCommandHook =
        serde_json::from_str(r#"{"command": "exit 2"}"#).expect("test operation should succeed");
    let outcome =
        execute_bash_command_hook(&hook, "").await.expect("test operation should succeed");
    assert_eq!(outcome.exit_code, Some(2));
    assert!(outcome.blocked);
}

#[tokio::test]
async fn bash_hook_timeout_covers_stdin_write() {
    let hook: BashCommandHook =
        serde_json::from_str(r#"{"command": "sleep 0.4", "timeout": 0.02}"#)
            .expect("test operation should succeed");
    let input = "x".repeat(1024 * 1024);

    let result = tokio::time::timeout(
        std::time::Duration::from_millis(100),
        execute_bash_command_hook(&hook, input),
    )
    .await;

    assert!(result.is_ok(), "hook execution should not hang while writing stdin");
    assert!(matches!(
        result.expect("timeout checked"),
        Err(BashHookError::Timeout { timeout: _, source: _ })
    ));
}

#[tokio::test]
async fn bash_hook_captures_stderr() {
    let hook: BashCommandHook = serde_json::from_str(r#"{"command": "echo oops 1>&2"}"#)
        .expect("test operation should succeed");
    let outcome =
        execute_bash_command_hook(&hook, "").await.expect("test operation should succeed");
    assert_eq!(outcome.exit_code, Some(0));
    assert!(outcome.stderr.contains("oops"));
}
