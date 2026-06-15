use std::path::{Path, PathBuf};
use std::sync::Arc;

use aries_core::ext::hook::input::{PostToolUseHookInput, PreToolUseHookInput};
use aries_core::ext::hook::{HookDecision, HooksExecutor};
use rig_core::agent::{HookAction, PromptHook, ToolCallHookAction};
use rig_core::completion::{CompletionModel, CompletionResponse, Message};
use serde_json::Value;

#[derive(Clone)]
pub struct SessionPromptHook {
    executor: Arc<HooksExecutor>,
    session_id: String,
    cwd: PathBuf,
}

impl SessionPromptHook {
    pub fn new(
        executor: Arc<HooksExecutor>,
        session_id: impl Into<String>,
        cwd: impl AsRef<Path>,
    ) -> Self {
        let session_id = session_id.into();
        let cwd = cwd.as_ref().to_path_buf();
        Self { executor, session_id, cwd }
    }
}

impl<M> PromptHook<M> for SessionPromptHook
where
    M: CompletionModel,
{
    async fn on_completion_call(&self, _prompt: &Message, _history: &[Message]) -> HookAction {
        HookAction::cont()
    }

    async fn on_completion_response(
        &self,
        _prompt: &Message,
        _response: &CompletionResponse<M::Response>,
    ) -> HookAction {
        HookAction::cont()
    }

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
            cwd: self.cwd.clone(),
            permission_mode: None,
            agent_id: None,
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
            cwd: self.cwd.clone(),
            permission_mode: None,
            agent_id: None,
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

    async fn on_text_delta(&self, _text_delta: &str, _aggregated_text: &str) -> HookAction {
        HookAction::cont()
    }

    async fn on_tool_call_delta(
        &self,
        _tool_call_id: &str,
        _internal_call_id: &str,
        _tool_name: Option<&str>,
        _tool_call_delta: &str,
    ) -> HookAction {
        HookAction::cont()
    }

    async fn on_stream_completion_response_finish(
        &self,
        _prompt: &Message,
        _response: &<M as CompletionModel>::StreamingResponse,
    ) -> HookAction {
        HookAction::cont()
    }
}
