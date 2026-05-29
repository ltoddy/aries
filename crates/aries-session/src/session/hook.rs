use std::path::PathBuf;
use std::sync::Arc;

use aries_core::ext::hook::input::{PostToolUseHookInput, PreToolUseHookInput};
use aries_core::ext::hook::{HookDecision, HooksExecutor};
use rig_core::agent::{HookAction, PromptHook, ToolCallHookAction};
use rig_core::completion::CompletionModel;
use serde_json::Value;

#[derive(Clone)]
pub struct SessionPromptHook {
    executor: Arc<HooksExecutor>,
    session_id: String,
    cwd: PathBuf,
    transcript_path: PathBuf,
}

impl SessionPromptHook {
    pub fn new(
        executor: Arc<HooksExecutor>,
        session_id: impl Into<String>,
        cwd: impl Into<PathBuf>,
        transcript_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            executor,
            session_id: session_id.into(),
            cwd: cwd.into(),
            transcript_path: transcript_path.into(),
        }
    }
}

impl<M> PromptHook<M> for SessionPromptHook
where
    M: CompletionModel,
{
    async fn on_tool_call(
        &self,
        tool_name: &str,
        tool_call_id: Option<String>,
        _internal_call_id: &str,
        args: &str,
    ) -> ToolCallHookAction {
        let tool_input: Value =
            serde_json::from_str(args).unwrap_or_else(|_| Value::String(args.to_owned()));

        let input = PreToolUseHookInput {
            session_id: self.session_id.clone(),
            transcript_path: self.transcript_path.clone(),
            cwd: self.cwd.clone(),
            permission_mode: None,
            agent_id: None,
            agent_type: None,
            hook_event_name: "PreToolUse".to_owned(),
            tool_name: tool_name.to_owned(),
            tool_input,
            tool_use_id: tool_call_id.unwrap_or_default(),
        };

        match self.executor.fire_pre_tool_use(&input).await {
            HookDecision::Continue => ToolCallHookAction::cont(),
            HookDecision::Terminate { reason } => ToolCallHookAction::skip(reason),
        }
    }

    async fn on_tool_result(
        &self,
        tool_name: &str,
        tool_call_id: Option<String>,
        _internal_call_id: &str,
        args: &str,
        result: &str,
    ) -> HookAction {
        let tool_input: Value =
            serde_json::from_str(args).unwrap_or_else(|_| Value::String(args.to_owned()));
        let tool_response: Value =
            serde_json::from_str(result).unwrap_or_else(|_| Value::String(result.to_owned()));

        let input = PostToolUseHookInput {
            session_id: self.session_id.clone(),
            transcript_path: self.transcript_path.clone(),
            cwd: self.cwd.clone(),
            permission_mode: None,
            agent_id: None,
            agent_type: None,
            hook_event_name: "PostToolUse".to_owned(),
            tool_name: tool_name.to_owned(),
            tool_input,
            tool_response,
            tool_use_id: tool_call_id.unwrap_or_default(),
        };

        match self.executor.fire_post_tool_use(&input).await {
            HookDecision::Continue => HookAction::cont(),
            HookDecision::Terminate { reason } => HookAction::terminate(reason),
        }
    }
}
