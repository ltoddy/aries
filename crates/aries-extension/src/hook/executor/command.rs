use std::io;
use std::process::Stdio;
use std::time::{Duration, TryFromFloatSecsError};

use thiserror::Error;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::error::Elapsed;

use crate::hook::definition::BashCommandHook;
use crate::hook::executor::output::HookOutput;

const DEFAULT_HOOK_TIMEOUT_SECS: f64 = 60.0;

pub async fn execute_bash_command_hook(
    hook: &BashCommandHook,
    stdin_payload: impl AsRef<[u8]>,
) -> Result<BashHookOutcome, BashHookError> {
    let timeout = hook.timeout.unwrap_or(DEFAULT_HOOK_TIMEOUT_SECS);
    let timeout_secs = if timeout.is_finite() { timeout.max(0.0) } else { timeout };
    let timeout_duration = Duration::try_from_secs_f64(timeout_secs)
        .map_err(|source| BashHookError::invalid_timeout(timeout, source))?;
    let (program, arg) = hook.shell.unwrap_or_default().invocation();

    let mut child = Command::new(program)
        .arg(arg)
        .arg(&hook.command)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(BashHookError::spawn)?;

    let stdin = child.stdin.take();
    let stdin_payload = stdin_payload.as_ref();

    let output = match tokio::time::timeout(timeout_duration, async move {
        let write_stdin = async move {
            if let Some(mut stdin) = stdin {
                if !stdin_payload.is_empty() {
                    stdin.write_all(stdin_payload).await.map_err(BashHookError::stdin)?;
                }
                stdin.shutdown().await.map_err(BashHookError::stdin)?;
            }
            Ok::<_, BashHookError>(())
        };
        let wait_output =
            async move { child.wait_with_output().await.map_err(BashHookError::wait) };

        let ((), output) = tokio::try_join!(write_stdin, wait_output)?;
        Ok::<_, BashHookError>(output)
    })
    .await
    {
        Ok(res) => res?,
        Err(elapsed) => {
            return Err(BashHookError::timeout(timeout_duration, elapsed));
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
    pub fn new(
        exit_code: Option<i32>,
        stdout: impl Into<String>,
        stderr: impl Into<String>,
        blocked: bool,
    ) -> Self {
        Self { exit_code, stdout: stdout.into(), stderr: stderr.into(), blocked }
    }

    pub fn output(self, hook_event_name: impl AsRef<str>) -> BashHookOutput {
        let hook_event_name = hook_event_name.as_ref();
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
            return BashHookOutput::Continue { context: Some(self.stdout) };
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
    #[error("invalid hook timeout {timeout}: {source}")]
    InvalidTimeout { timeout: f64, source: TryFromFloatSecsError },
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

impl BashHookError {
    pub fn invalid_timeout(timeout: f64, source: TryFromFloatSecsError) -> Self {
        Self::InvalidTimeout { timeout, source }
    }

    pub fn spawn(source: io::Error) -> Self {
        Self::Spawn(source)
    }

    pub fn stdin(source: io::Error) -> Self {
        Self::Stdin(source)
    }

    pub fn wait(source: io::Error) -> Self {
        Self::Wait(source)
    }

    pub fn timeout(timeout: Duration, source: Elapsed) -> Self {
        Self::Timeout { timeout, source }
    }
}
