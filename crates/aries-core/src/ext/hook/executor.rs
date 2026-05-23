use std::collections::HashMap;
use std::fmt::Debug;
use std::io;
use std::process::Stdio;
use std::time::Duration;

use serde::Serialize;
use thiserror::Error;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::error::Elapsed;
use tracing::warn;

use crate::ext::hook::HooksPreset;
use crate::ext::hook::input::{PostToolUseHookInput, PreToolUseHookInput};
use crate::ext::hook::preset::{BashCommandHook, HookCommand, HookEvent, HookMatcher};

const DEFAULT_HOOK_TIMEOUT_SECS: f64 = 60.0;

#[derive(Debug, Clone)]
pub enum HookDecision {
    Continue,
    Terminate { reason: String },
}

impl HookDecision {
    pub fn is_terminate(&self) -> bool {
        matches!(self, HookDecision::Terminate { .. })
    }
}

pub struct HooksExecutor {
    hooks: HashMap<HookEvent, Vec<HookMatcher>>,
}

impl HooksExecutor {
    pub fn new(presets: Vec<HooksPreset>) -> Self {
        let mut hooks = HashMap::<HookEvent, Vec<HookMatcher>>::new();

        for preset in presets {
            let settings = preset.hooks.0;
            hooks.extend(settings);
        }

        Self { hooks }
    }

    pub async fn fire_pre_tool_use<ToolInput>(
        &self,
        input: &PreToolUseHookInput<ToolInput>,
    ) -> HookDecision
    where
        ToolInput: Serialize + Clone + Debug,
    {
        let payload = match serde_json::to_string(input) {
            Ok(s) => s,
            Err(err) => {
                warn!("failed to serialize PreToolUseHookInput: {err}");
                return HookDecision::Continue;
            },
        };

        let tool_name = input.tool_name.as_str();
        let Some(matchers) = self.hooks.get(&HookEvent::PreToolUse) else {
            return HookDecision::Continue;
        };

        for matcher in matchers {
            match matcher.matches(tool_name) {
                Ok(true) => {},
                Ok(false) => continue,
                Err(err) => {
                    warn!("invalid hook matcher, skipped: {err}");
                    continue;
                },
            }

            for hook in &matcher.hooks {
                match hook {
                    HookCommand::Command(bash) => {
                        match execute_bash_command_hook(bash, &payload).await {
                            Ok(outcome) if outcome.blocked => {
                                let reason = if outcome.stderr.trim().is_empty() {
                                    format!(
                                        "PreToolUse hook blocked tool {:?} (exit code 2)",
                                        tool_name
                                    )
                                } else {
                                    outcome.stderr.trim().to_string()
                                };
                                return HookDecision::Terminate { reason };
                            },
                            Ok(_) => {},
                            Err(err) => {
                                warn!("bash hook execution failed for tool {:?}: {err}", tool_name);
                            },
                        }
                    },
                    HookCommand::Prompt(_) => {},
                    HookCommand::Agent(_) => {},
                    HookCommand::Http(_) => {},
                }
            }
        }

        HookDecision::Continue
    }

    pub async fn fire_post_tool_use<ToolInput, ToolResponse>(
        &self,
        input: &PostToolUseHookInput<ToolInput, ToolResponse>,
    ) -> HookDecision
    where
        ToolInput: Serialize + Clone + Debug,
        ToolResponse: Serialize + Clone + Debug,
    {
        let payload = match serde_json::to_string(input) {
            Ok(s) => s,
            Err(err) => {
                warn!("failed to serialize PostToolUseHookInput: {err}");
                return HookDecision::Continue;
            },
        };

        let tool_name = input.tool_name.as_str();
        let Some(matchers) = self.hooks.get(&HookEvent::PostToolUse) else {
            return HookDecision::Continue;
        };

        for matcher in matchers {
            match matcher.matches(tool_name) {
                Ok(true) => {},
                Ok(false) => continue,
                Err(err) => {
                    warn!("invalid hook matcher, skipped: {err}");
                    continue;
                },
            }

            for hook in &matcher.hooks {
                match hook {
                    HookCommand::Command(bash) => {
                        match execute_bash_command_hook(bash, &payload).await {
                            Ok(outcome) if outcome.blocked => {
                                let reason = if outcome.stderr.trim().is_empty() {
                                    format!(
                                        "PostToolUse hook blocked tool {:?} (exit code 2)",
                                        tool_name
                                    )
                                } else {
                                    outcome.stderr.trim().to_string()
                                };
                                return HookDecision::Terminate { reason };
                            },
                            Ok(_) => {},
                            Err(err) => {
                                warn!("bash hook execution failed for tool {:?}: {err}", tool_name);
                            },
                        }
                    },
                    HookCommand::Prompt(_) => {},
                    HookCommand::Agent(_) => {},
                    HookCommand::Http(_) => {},
                }
            }
        }

        HookDecision::Continue
    }
}

#[derive(Debug, Clone)]
pub struct BashHookOutcome {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub blocked: bool,
}

#[derive(Debug, Error)]
pub enum BashHookError {
    #[error("failed to spawn shell process: {0}")]
    Spawn(#[source] io::Error),
    #[error("failed to write stdin to hook process: {0}")]
    Stdin(#[source] io::Error),
    #[error("failed to wait hook process: {0}")]
    Wait(#[source] io::Error),
    #[error("hook timed out after {timeout:?}")]
    Timeout {
        timeout: Duration,
        #[source]
        source: Elapsed,
    },
}

pub async fn execute_bash_command_hook(
    hook: &BashCommandHook,
    stdin_payload: &str,
) -> Result<BashHookOutcome, BashHookError> {
    let (program, arg) = hook.shell.unwrap_or_default().invocation();

    let mut child = Command::new(program)
        .arg(arg)
        .arg(&hook.command)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(BashHookError::Spawn)?;

    if !stdin_payload.is_empty() {
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(stdin_payload.as_bytes()).await.map_err(BashHookError::Stdin)?;
            stdin.shutdown().await.map_err(BashHookError::Stdin)?;
        }
    } else {
        drop(child.stdin.take());
    }

    let timeout_duration =
        Duration::from_secs_f64(hook.timeout.unwrap_or(DEFAULT_HOOK_TIMEOUT_SECS).max(0.0));

    let output = match tokio::time::timeout(timeout_duration, child.wait_with_output()).await {
        Ok(res) => res.map_err(BashHookError::Wait)?,
        Err(elapsed) => {
            return Err(BashHookError::Timeout { timeout: timeout_duration, source: elapsed });
        },
    };

    let exit_code = output.status.code();
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let blocked = exit_code == Some(2);

    Ok(BashHookOutcome { exit_code, stdout, stderr, blocked })
}
