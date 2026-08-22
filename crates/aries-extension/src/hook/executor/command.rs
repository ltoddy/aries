use std::io;
use std::process::Stdio;
use std::time::Duration;

use thiserror::Error;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::error::Elapsed;

use crate::hook::definition::BashCommandHook;
use crate::hook::executor::output::HookOutput;

const DEFAULT_HOOK_TIMEOUT_SECS: f64 = 60.0;

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

    Ok(BashHookOutcome::new(exit_code, stdout, stderr, blocked))
}

#[derive(Debug, Clone)]
pub struct BashHookOutcome {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub blocked: bool,
}

impl BashHookOutcome {
    pub fn new(exit_code: Option<i32>, stdout: String, stderr: String, blocked: bool) -> Self {
        Self { exit_code, stdout, stderr, blocked }
    }

    pub fn output(&self, hook_event_name: &str) -> BashHookOutput {
        if self.blocked {
            let reason = if self.stderr.trim().is_empty() {
                format!("{hook_event_name} hook returned exit code 2")
            } else {
                format!("{hook_event_name} hook returned exit code 2: {}", self.stderr.trim())
            };
            return BashHookOutput::Terminate { reason };
        }

        if self.exit_code != Some(0) {
            return BashHookOutput::Continue { context: None };
        }

        let stdout = self.stdout.trim();
        if stdout.is_empty() {
            return BashHookOutput::Continue { context: None };
        }

        if let Ok(json) = serde_json::from_str::<HookOutput>(stdout) {
            if !json.should_continue {
                let reason = json
                    .stop_reason
                    .unwrap_or_else(|| format!("{hook_event_name} hook returned continue: false"));
                return BashHookOutput::Terminate { reason };
            }
            return BashHookOutput::Continue { context: json.additional_context(hook_event_name) };
        }

        if plain_stdout_adds_context(hook_event_name) {
            return BashHookOutput::Continue { context: Some(self.stdout.clone()) };
        }

        BashHookOutput::Continue { context: None }
    }
}

#[derive(Debug, Clone)]
pub enum BashHookOutput {
    Continue { context: Option<String> },
    Terminate { reason: String },
}

fn plain_stdout_adds_context(hook_event_name: &str) -> bool {
    matches!(hook_event_name, "SessionStart" | "UserPromptSubmit" | "UserPromptExpansion")
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
