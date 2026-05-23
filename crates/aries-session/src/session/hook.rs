use std::sync::Arc;

use aries_core::ext::hook::input::PostToolUseHookInput;
use aries_core::ext::hook::{HookDecision, HooksExecutor};
use rig::agent::{HookAction, PromptHook};
use rig::completion::CompletionModel;
use serde_json::Value;

#[derive(Clone)]
pub struct SessionPromptHook {
    executor: Arc<HooksExecutor>,
    session_id: String,
    cwd: std::path::PathBuf,
    transcript_path: std::path::PathBuf,
}

impl SessionPromptHook {
    pub fn new(
        executor: Arc<HooksExecutor>,
        session_id: impl Into<String>,
        cwd: impl Into<std::path::PathBuf>,
        transcript_path: impl Into<std::path::PathBuf>,
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
