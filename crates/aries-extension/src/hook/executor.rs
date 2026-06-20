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
use tracing::{info, warn};

use crate::hook::input::{
    HookInput, PostCompactHookInput, PostToolUseFailureHookInput, PostToolUseHookInput,
    PreCompactHookInput, PreToolUseHookInput, SessionEndHookInput, SessionStartHookInput,
    StopFailureHookInput, StopHookInput, SubagentStartHookInput, SubagentStopHookInput,
    UserPromptSubmitHookInput,
};
use crate::hook::preset::{BashCommandHook, HookCommand, HookEvent, HookMatcher, HooksPreset};

const DEFAULT_HOOK_TIMEOUT_SECS: f64 = 60.0;

#[derive(Debug, Clone)]
pub enum HookDecision {
    Continue,
    Terminate { reason: String },
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

    pub async fn fire_post_compact(&self, input: PostCompactHookInput) {
        let hook_event_name = input.hook_event_name();
        let input = serde_json::to_string(&input).unwrap();
        info!(event = hook_event_name, input = %input, "received hook event");

        let Some(matchers) = self.hooks.get(&HookEvent::PostCompact) else { return };

        for matcher in matchers {
            if let HookDecision::Terminate { reason } =
                Self::fire_hooks(&matcher.hooks, input.clone(), hook_event_name).await
            {
                warn!(event = hook_event_name, %reason, "hook exited with code 2");
            }
        }
    }

    pub async fn fire_post_tool_use_failure<ToolInput>(
        &self,
        input: PostToolUseFailureHookInput<ToolInput>,
    ) where
        ToolInput: Serialize + Clone + Debug,
    {
        let hook_event_name = input.hook_event_name();
        let tool_name = input.tool_name.as_str();
        let input = serde_json::to_string(&input).unwrap();
        info!(event = hook_event_name, input = %input, "received hook event");

        let Some(matchers) = self.hooks.get(&HookEvent::PostToolUseFailure) else { return };

        for matcher in matchers {
            match matcher.matches(tool_name) {
                Ok(true) => {
                    if let HookDecision::Terminate { reason } =
                        Self::fire_hooks(&matcher.hooks, input.clone(), hook_event_name).await
                    {
                        warn!(event = hook_event_name, %reason, "hook exited with code 2");
                    }
                },
                Err(err) => warn!("invalid hook matcher, skipped: {err}"),
                _ => {},
            }
        }
    }

    pub async fn fire_post_tool_use<ToolInput, ToolResponse>(
        &self,
        input: PostToolUseHookInput<ToolInput, ToolResponse>,
    ) where
        ToolInput: Clone + Debug + Default + Serialize,
        ToolResponse: Clone + Debug + Default + Serialize,
    {
        let hook_event_name = input.hook_event_name();
        let tool_name = input.tool_name.as_str();
        let input = serde_json::to_string(&input).unwrap();
        info!(event = hook_event_name, input = %input, "received hook event");

        let Some(matchers) = self.hooks.get(&HookEvent::PostToolUse) else { return };

        for matcher in matchers {
            match matcher.matches(tool_name) {
                Ok(true) => {
                    if let HookDecision::Terminate { reason } =
                        Self::fire_hooks(&matcher.hooks, input.clone(), hook_event_name).await
                    {
                        warn!(event = hook_event_name, %reason, "hook exited with code 2");
                    }
                },
                Err(err) => warn!("invalid hook matcher, skipped: {err}"),
                _ => {},
            }
        }
    }

    pub async fn fire_pre_compact(&self, input: PreCompactHookInput) -> HookDecision {
        let hook_event_name = input.hook_event_name();
        let input = serde_json::to_string(&input).unwrap();
        info!(event = hook_event_name, input = %input, "received hook event");

        let Some(matchers) = self.hooks.get(&HookEvent::PreCompact) else {
            return HookDecision::Continue;
        };

        for matcher in matchers {
            if let HookDecision::Terminate { reason } =
                Self::fire_hooks(&matcher.hooks, input.clone(), hook_event_name).await
            {
                warn!(event = hook_event_name, %reason, "hook blocked with exit code 2");
                return HookDecision::Terminate { reason };
            }
        }

        HookDecision::Continue
    }

    pub async fn fire_pre_tool_use<ToolInput>(
        &self,
        input: PreToolUseHookInput<ToolInput>,
    ) -> HookDecision
    where
        ToolInput: Clone + Debug + Default + Serialize,
    {
        let hook_event_name = input.hook_event_name();
        let tool_name = input.tool_name.clone();
        let input = serde_json::to_string(&input).unwrap();
        info!(event = hook_event_name, input = %input, "received hook event");
        let Some(matchers) = self.hooks.get(&HookEvent::PreToolUse) else {
            return HookDecision::Continue;
        };

        for matcher in matchers {
            match matcher.matches(&tool_name) {
                Ok(true) => {
                    if let HookDecision::Terminate { reason } =
                        Self::fire_hooks(&matcher.hooks, input.clone(), hook_event_name).await
                    {
                        warn!(event = hook_event_name, %reason, "hook blocked with exit code 2");
                        return HookDecision::Terminate { reason };
                    }
                },
                Err(err) => warn!("invalid hook matcher, skipped: {err}"),
                _ => {},
            }
        }

        HookDecision::Continue
    }

    pub async fn fire_session_end(&self, input: SessionEndHookInput) {
        let hook_event_name = input.hook_event_name();
        let input = serde_json::to_string(&input).unwrap();
        info!(event = hook_event_name, input = %input, "received hook event");

        let Some(matchers) = self.hooks.get(&HookEvent::SessionEnd) else { return };

        for matcher in matchers {
            if let HookDecision::Terminate { reason } =
                Self::fire_hooks(&matcher.hooks, input.clone(), hook_event_name).await
            {
                warn!(event = hook_event_name, %reason, "hook exited with code 2");
            }
        }
    }

    pub async fn fire_session_start(&self, input: SessionStartHookInput) {
        let hook_event_name = input.hook_event_name();
        let input = serde_json::to_string(&input).unwrap();
        info!(event = hook_event_name, input = %input, "received hook event");

        let Some(matchers) = self.hooks.get(&HookEvent::SessionStart) else { return };

        for matcher in matchers {
            if let HookDecision::Terminate { reason } =
                Self::fire_hooks(&matcher.hooks, input.clone(), hook_event_name).await
            {
                warn!(event = hook_event_name, %reason, "hook exited with code 2");
            }
        }
    }

    pub async fn fire_stop_failure(&self, input: StopFailureHookInput) {
        let hook_event_name = input.hook_event_name();
        let input = serde_json::to_string(&input).unwrap();
        info!(event = hook_event_name, input = %input, "received hook event");

        let Some(matchers) = self.hooks.get(&HookEvent::StopFailure) else { return };

        for matcher in matchers {
            if let HookDecision::Terminate { reason } =
                Self::fire_hooks(&matcher.hooks, input.clone(), hook_event_name).await
            {
                warn!(event = hook_event_name, %reason, "hook exited with code 2");
            }
        }
    }

    /// 在文档中对于 stop hook 是可以被阻止的: `Prevents Claude from stopping, continues the conversation`. 没太理解, 先记录个 todo
    pub async fn fire_stop(&self, input: StopHookInput) -> HookDecision {
        let hook_event_name = input.hook_event_name();
        let input = serde_json::to_string(&input).unwrap();
        info!(event = hook_event_name, input = %input, "received hook event");

        let Some(matchers) = self.hooks.get(&HookEvent::Stop) else {
            return HookDecision::Continue;
        };

        for matcher in matchers {
            if let HookDecision::Terminate { reason } =
                Self::fire_hooks(&matcher.hooks, input.clone(), hook_event_name).await
            {
                warn!(event = hook_event_name, %reason, "hook exited with code 2");
                return HookDecision::Terminate { reason };
            }
        }

        HookDecision::Continue
    }

    pub async fn fire_subagent_start(&self, input: SubagentStartHookInput) {
        let hook_event_name = input.hook_event_name();
        let input = serde_json::to_string(&input).unwrap();
        info!(event = hook_event_name, input = %input, "received hook event");

        let Some(matchers) = self.hooks.get(&HookEvent::SubagentStart) else { return };

        for matcher in matchers {
            if let HookDecision::Terminate { reason } =
                Self::fire_hooks(&matcher.hooks, input.clone(), hook_event_name).await
            {
                warn!(event = hook_event_name, %reason, "hook exited with code 2");
            }
        }
    }

    /// 在文档中对于 subagent stop hook 是可以被阻止的: `Prevents the subagent from stopping` 没太理解, 先记录一个 todo
    pub async fn fire_subagent_stop(&self, input: SubagentStopHookInput) -> HookDecision {
        let hook_event_name = input.hook_event_name();
        let input = serde_json::to_string(&input).unwrap();
        info!(event = hook_event_name, input = %input, "received hook event");

        let Some(matchers) = self.hooks.get(&HookEvent::SubagentStop) else {
            return HookDecision::Continue;
        };

        for matcher in matchers {
            if let HookDecision::Terminate { reason } =
                Self::fire_hooks(&matcher.hooks, input.clone(), hook_event_name).await
            {
                warn!(event = hook_event_name, %reason, "hook exited with code 2");
                return HookDecision::Terminate { reason };
            }
        }

        HookDecision::Continue
    }

    pub async fn fire_user_prompt_submit(&self, input: UserPromptSubmitHookInput) -> HookDecision {
        let hook_event_name = input.hook_event_name();
        let input = serde_json::to_string(&input).unwrap();
        info!(event = hook_event_name, input = %input, "received hook event");

        let Some(matchers) = self.hooks.get(&HookEvent::UserPromptSubmit) else {
            return HookDecision::Continue;
        };

        for matcher in matchers {
            if let HookDecision::Terminate { reason } =
                Self::fire_hooks(&matcher.hooks, input.clone(), hook_event_name).await
            {
                warn!(event = hook_event_name, %reason, "hook exited with code 2");
                return HookDecision::Terminate { reason };
            }
        }

        HookDecision::Continue
    }

    async fn fire_hooks(
        hooks: &[HookCommand],
        input: impl Into<String>,
        hook_event_name: &str,
    ) -> HookDecision {
        let payload = input.into();

        for hook in hooks {
            match hook {
                HookCommand::Command(bash) => {
                    match execute_bash_command_hook(bash, &payload).await {
                        Ok(outcome) if outcome.blocked => {
                            let reason = if outcome.stderr.trim().is_empty() {
                                format!("{hook_event_name} hook returned exit code 2")
                            } else {
                                format!(
                                    "{hook_event_name} hook returned exit code 2: {}",
                                    outcome.stderr.trim()
                                )
                            };
                            return HookDecision::Terminate { reason };
                        },
                        Err(err) => {
                            let reason = format!("bash hook execute failed: {err}");
                            return HookDecision::Terminate { reason };
                        },
                        _ => continue,
                    }
                },
                HookCommand::Prompt(_) => {},
                HookCommand::Agent(_) => {},
                HookCommand::Http(_) => {},
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
    stdin_payload: impl Into<String>,
) -> Result<BashHookOutcome, BashHookError> {
    let stdin_payload = stdin_payload.into();
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
