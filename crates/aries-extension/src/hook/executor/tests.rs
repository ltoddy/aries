// This file contains tests generated with AI assistance.

use super::*;
use crate::hook::definition::BashCommandHook;

#[tokio::test]
async fn bash_hook_exit_zero_does_not_block() {
    let hook: BashCommandHook = serde_json::from_str(r#"{"command": "echo hello"}"#).unwrap();
    let outcome = execute_bash_command_hook(&hook, "").await.unwrap();
    assert_eq!(outcome.exit_code, Some(0));
    assert!(!outcome.blocked);
    assert!(outcome.stdout.contains("hello"));
}

#[tokio::test]
async fn bash_hook_exit_two_blocks() {
    let hook: BashCommandHook = serde_json::from_str(r#"{"command": "exit 2"}"#).unwrap();
    let outcome = execute_bash_command_hook(&hook, "").await.unwrap();
    assert_eq!(outcome.exit_code, Some(2));
    assert!(outcome.blocked);
}

#[tokio::test]
async fn bash_hook_captures_stderr() {
    let hook: BashCommandHook = serde_json::from_str(r#"{"command": "echo oops 1>&2"}"#).unwrap();
    let outcome = execute_bash_command_hook(&hook, "").await.unwrap();
    assert_eq!(outcome.exit_code, Some(0));
    assert!(outcome.stderr.contains("oops"));
}
