// This file contains tests generated with AI assistance.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use tempfile::TempDir;

use super::*;
use crate::hook::executor::execute_bash_command_hook;
use crate::hook::input::PreToolUseHookInput;
use crate::hook::preset::{
    BashCommandHook, HookCommand, HookEvent, HookMatcher, HookMatcherError, HooksSettings,
    ParseHooksFileError, ShellType,
};

/// 在 `root/.agents/hooks/` 下写入一个 hooks.json，事件名为 key。
fn write_hooks_json(root: &Path, event: &str) {
    let dir = root.join(".agents").join("hooks");
    fs::create_dir_all(&dir).unwrap();
    let content = format!(
        r#"{{"description": "{event} demo", "hooks": {{"{event}": [{{"hooks": [{{"type": "command", "command": "echo hi"}}]}}]}}}}"#
    );
    fs::write(dir.join("hooks.json"), content).unwrap();
}

fn command_hook(command: &str) -> HookCommand {
    serde_json::from_str(&format!(r#"{{"type": "command", "command": "{command}"}}"#)).unwrap()
}

#[test]
fn deserializes_hook_events() {
    let event: HookEvent = serde_json::from_str(r#""PreToolUse""#).unwrap();
    assert_eq!(event, HookEvent::PreToolUse);
    let event: HookEvent = serde_json::from_str(r#""PostToolUse""#).unwrap();
    assert_eq!(event, HookEvent::PostToolUse);
    let event: HookEvent = serde_json::from_str(r#""Stop""#).unwrap();
    assert_eq!(event, HookEvent::Stop);
    let event: HookEvent = serde_json::from_str(r#""SessionStart""#).unwrap();
    assert_eq!(event, HookEvent::SessionStart);
}

#[test]
fn deserializes_hooks_settings() {
    let json = r#"{
        "PreToolUse": [{"matcher": "Write", "hooks": [{"type": "command", "command": "echo hi"}]}],
        "Stop": []
    }"#;
    let settings: HooksSettings = serde_json::from_str(json).unwrap();
    assert_eq!(settings.0.len(), 2);
    assert!(settings.0.contains_key(&HookEvent::PreToolUse));
    assert!(settings.0.contains_key(&HookEvent::Stop));
}

#[test]
fn deserializes_hook_command_variants() {
    let cmd: HookCommand =
        serde_json::from_str(r#"{"type": "command", "command": "echo hi"}"#).unwrap();
    assert!(matches!(cmd, HookCommand::Command(_)));

    let prompt: HookCommand =
        serde_json::from_str(r#"{"type": "prompt", "prompt": "assess"}"#).unwrap();
    assert!(matches!(prompt, HookCommand::Prompt(_)));

    let agent: HookCommand =
        serde_json::from_str(r#"{"type": "agent", "prompt": "verify"}"#).unwrap();
    assert!(matches!(agent, HookCommand::Agent(_)));

    let http: HookCommand =
        serde_json::from_str(r#"{"type": "http", "url": "https://example.com"}"#).unwrap();
    assert!(matches!(http, HookCommand::Http(_)));
}

#[test]
fn matcher_matches_all_when_absent_or_wildcard() {
    assert!(HookMatcher { matcher: None, hooks: vec![] }.matches("Write").unwrap());
    assert!(HookMatcher { matcher: Some("*".to_owned()), hooks: vec![] }.matches("Bash").unwrap());
    assert!(
        HookMatcher { matcher: Some("   ".to_owned()), hooks: vec![] }.matches("Bash").unwrap()
    );
}

#[test]
fn matcher_matches_tool_name_regex() {
    let matcher = HookMatcher { matcher: Some("(Read|Edit)".to_owned()), hooks: vec![] };
    assert!(matcher.matches("Read").unwrap());
    assert!(matcher.matches("Edit").unwrap());
    assert!(!matcher.matches("Write").unwrap());
}

#[test]
fn matcher_anchors_alternation() {
    let matcher = HookMatcher { matcher: Some("Edit|Write".to_owned()), hooks: vec![] };
    assert!(matcher.matches("Edit").unwrap());
    assert!(matcher.matches("Write").unwrap());
    assert!(!matcher.matches("Editor").unwrap());
    assert!(!matcher.matches("Rewrite").unwrap());
}

#[test]
fn matcher_rejects_invalid_regex() {
    let matcher = HookMatcher { matcher: Some("(unclosed".to_owned()), hooks: vec![] };
    assert!(matches!(matcher.matches("Write"), Err(HookMatcherError::InvalidRegex { .. })));
}

#[test]
fn shell_type_invocation() {
    assert_eq!(ShellType::Bash.invocation(), ("bash", "-c"));
    assert_eq!(ShellType::Sh.invocation(), ("sh", "-c"));
    assert_eq!(ShellType::Zsh.invocation(), ("zsh", "-c"));
    assert_eq!(ShellType::Powershell.invocation(), ("powershell", "-Command"));
}

#[tokio::test]
async fn parses_hooks_preset() {
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("hooks.json");
    fs::write(
        &file,
        r#"{
            "description": "demo",
            "hooks": {
                "PreToolUse": [
                    {"matcher": "Write", "hooks": [{"type": "command", "command": "echo hi"}]}
                ]
            }
        }"#,
    )
    .unwrap();

    let preset = HooksPreset::parse(&file).await.unwrap();
    assert_eq!(preset.description.as_deref(), Some("demo"));
    assert_eq!(preset.hooks.0.len(), 1);
    assert!(preset.hooks.0.contains_key(&HookEvent::PreToolUse));
}

#[tokio::test]
async fn parse_reports_missing_file() {
    let err = HooksPreset::parse("/nonexistent/hooks.json").await.unwrap_err();
    assert!(matches!(err, ParseHooksFileError::Io(_)));
}

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

fn pre_tool_use_input(tool_name: &str) -> PreToolUseHookInput<String> {
    PreToolUseHookInput::new("session-1", "/tmp", tool_name, "{}".to_owned(), "tool-use-1")
}

#[tokio::test]
async fn executor_continues_when_no_hooks_registered() {
    let executor = HooksExecutor::new(vec![]);
    let decision = executor.fire_pre_tool_use(pre_tool_use_input("Write")).await;
    assert!(matches!(decision, HookDecision::Continue));
}

#[tokio::test]
async fn executor_blocks_pre_tool_use_when_hook_exits_two() {
    let preset = HooksPreset {
        description: None,
        hooks: HooksSettings(HashMap::from([(
            HookEvent::PreToolUse,
            vec![HookMatcher {
                matcher: Some("Write".to_owned()),
                hooks: vec![command_hook("exit 2")],
            }],
        )])),
    };
    let executor = HooksExecutor::new(vec![preset]);

    let decision = executor.fire_pre_tool_use(pre_tool_use_input("Write")).await;
    match decision {
        HookDecision::Terminate { reason } => assert!(reason.contains("exit code 2")),
        HookDecision::Continue => panic!("expected hook to be blocked"),
    }
}

#[tokio::test]
async fn executor_skips_hook_when_matcher_does_not_match() {
    let preset = HooksPreset {
        description: None,
        hooks: HooksSettings(HashMap::from([(
            HookEvent::PreToolUse,
            vec![HookMatcher {
                matcher: Some("Bash".to_owned()),
                hooks: vec![command_hook("exit 2")],
            }],
        )])),
    };
    let executor = HooksExecutor::new(vec![preset]);

    let decision = executor.fire_pre_tool_use(pre_tool_use_input("Write")).await;
    assert!(matches!(decision, HookDecision::Continue));
}

#[tokio::test]
async fn load_finds_hooks_from_home_and_cwd() {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path().join("home");
    let cwd = tmp.path().join("cwd");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&cwd).unwrap();

    write_hooks_json(&home, "PreToolUse");
    write_hooks_json(&cwd, "Stop");

    let loader = HooksLoader::new(&cwd, &home);
    let presets = loader.load().await;
    assert_eq!(presets.len(), 2);

    let events: Vec<_> = presets.iter().flat_map(|p| p.hooks.0.keys()).collect();
    assert!(events.contains(&&HookEvent::PreToolUse));
    assert!(events.contains(&&HookEvent::Stop));
}

#[tokio::test]
async fn load_ignores_non_hooks_json() {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path().join("home");
    let cwd = tmp.path().join("cwd");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&cwd).unwrap();

    write_hooks_json(&home, "PreToolUse");
    let dir = cwd.join(".agents").join("hooks");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("other.json"), r#"{"description": "x", "hooks": {}}"#).unwrap();

    let loader = HooksLoader::new(&cwd, &home);
    let presets = loader.load().await;
    assert_eq!(presets.len(), 1);
}

#[tokio::test]
async fn load_returns_empty_when_no_roots_exist() {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path().join("home");
    let cwd = tmp.path().join("cwd");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&cwd).unwrap();

    let loader = HooksLoader::new(&cwd, &home);
    let presets = loader.load().await;
    assert!(presets.is_empty());
}
