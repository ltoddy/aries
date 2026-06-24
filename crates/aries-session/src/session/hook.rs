use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time;

use aries_core::tools::{AgentOutput, agent};
use aries_extension::hook::input::{
    PostToolUseFailureHookInput, PostToolUseHookInput, PreToolUseHookInput, SubagentStartHookInput,
    SubagentStopHookInput,
};
use aries_extension::hook::{HookDecision, HooksExecutor};
use aries_track::ToolCallRepository;
use parking_lot::Mutex;
use rig_core::agent::{HookAction, PromptHook, ToolCallHookAction};
use rig_core::completion::{CompletionModel, CompletionResponse, Message};
use serde_json::Value;
use toasty::Db;

#[derive(Clone)]
pub struct SessionPromptHook {
    executor: Arc<HooksExecutor>,
    session_id: String,
    cwd: PathBuf,
    transcript_path: PathBuf,
    agent_id: String,
    agent_type: String,
    last_tool_call_at: Arc<Mutex<Option<time::Instant>>>,

    tool_call_repo: ToolCallRepository,
}

impl SessionPromptHook {
    pub fn new(
        executor: Arc<HooksExecutor>,
        session_id: impl Into<String>,
        cwd: impl AsRef<Path>,
        transcript_path: impl AsRef<Path>,
        agent_id: impl Into<String>,
        agent_type: impl Into<String>,
        db: Db,
    ) -> Self {
        let session_id = session_id.into();
        let cwd = cwd.as_ref().to_path_buf();
        let transcript_path = transcript_path.as_ref().to_path_buf();
        let agent_id = agent_id.into();
        let agent_type = agent_type.into();

        let tool_call_repo = ToolCallRepository::new(db);

        Self {
            executor,
            session_id,
            cwd,
            transcript_path,
            agent_id,
            agent_type,
            last_tool_call_at: Default::default(),
            tool_call_repo,
        }
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
        _tool_call_id: Option<String>,
        internal_call_id: &str,
        args: &str,
    ) -> ToolCallHookAction {
        let call_at = time::Instant::now();

        let tool_input: Value =
            serde_json::from_str(args).unwrap_or_else(|_| Value::String(args.to_owned()));

        if tool_name == agent::NAME {
            let input = SubagentStartHookInput::new(&self.session_id, &self.cwd, "", "")
                .transcript_path(&self.transcript_path);
            self.executor.fire_subagent_start(input).await;
        }

        let input = PreToolUseHookInput::new(
            &self.session_id,
            &self.cwd,
            tool_name,
            tool_input,
            internal_call_id,
        )
        .transcript_path(&self.transcript_path)
        .agent_id(&self.agent_id)
        .agent_type(&self.agent_type);

        match self.executor.fire_pre_tool_use(input).await {
            HookDecision::Continue => {
                let mut last_tool_call_at = self.last_tool_call_at.lock();
                *last_tool_call_at = Some(call_at);
                ToolCallHookAction::cont()
            },
            HookDecision::Terminate { reason } => {
                let mut last_tool_call_at = self.last_tool_call_at.lock();
                *last_tool_call_at = None;
                ToolCallHookAction::terminate(reason)
            },
        }
    }

    async fn on_tool_result(
        &self,
        tool_name: &str,
        _tool_call_id: Option<String>,
        internal_call_id: &str,
        args: &str,
        result: &str,
    ) -> HookAction {
        let duration_ms = self
            .last_tool_call_at
            .lock()
            .take()
            .map(|started_at| started_at.elapsed().as_millis() as u64);

        let tool_input: Value =
            serde_json::from_str(args).unwrap_or_else(|_| Value::String(args.to_owned()));
        let tool_response: Value =
            serde_json::from_str(result).unwrap_or_else(|_| Value::String(result.to_owned()));

        let was_successful = !result.contains("ToolCallError");
        {
            let mut repo = self.tool_call_repo.clone();
            let session_id = self.session_id.clone();
            let internal_call_id = internal_call_id.to_owned();
            let tool_name = tool_name.to_owned();
            let args = args.to_owned();
            tokio::spawn(async move {
                let _ = repo
                    .create(
                        session_id,
                        internal_call_id,
                        tool_name,
                        args,
                        duration_ms,
                        was_successful,
                    )
                    .await;
            });
        }

        if !was_successful {
            let input = PostToolUseFailureHookInput::new(
                &self.session_id,
                &self.cwd,
                tool_name,
                &tool_input,
                internal_call_id,
                result,
            )
            .transcript_path(&self.transcript_path)
            .is_interrupt(false);
            let input = match duration_ms {
                Some(duration_ms) => input.duration_ms(duration_ms),
                None => input,
            };

            self.executor.fire_post_tool_use_failure(input).await;
            return HookAction::cont();
        }

        if tool_name == agent::NAME {
            let input = SubagentStopHookInput::new(
                &self.session_id,
                &self.cwd,
                false,
                &self.agent_id,
                &self.agent_type,
            )
            .transcript_path(&self.transcript_path);

            let input = match serde_json::from_value::<AgentOutput>(tool_response.clone()) {
                Ok(output) => input.last_assistant_message(output.result),
                Err(_) => input,
            };
            self.executor.fire_subagent_stop(input).await;
        }

        let input = PostToolUseHookInput::new(
            &self.session_id,
            &self.cwd,
            tool_name,
            &tool_input,
            &tool_response,
            internal_call_id,
        )
        .transcript_path(&self.transcript_path)
        .agent_id(&self.agent_id)
        .agent_type(&self.agent_type);
        let input = match duration_ms {
            Some(duration_ms) => input.duration_ms(duration_ms),
            None => input,
        };

        self.executor.fire_post_tool_use(input).await;

        HookAction::cont()
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
