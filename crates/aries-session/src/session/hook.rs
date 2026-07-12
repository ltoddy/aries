use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time;

use aries_extension::hook::input::{
    PostToolUseFailureHookInput, PostToolUseHookInput, PreToolUseHookInput, SubagentStartHookInput,
    SubagentStopHookInput,
};
use aries_extension::hook::{HookDecision, HooksExecutor};
use aries_persistence::ToolCallRepository;
use aries_tools::agent::AgentOutput;
use parking_lot::Mutex;
use rig_core::OneOrMany;
use rig_core::agent::{
    AgentHook, Flow, HookContext, InvalidToolCallContext, StepEvent, StepEventKind,
};
use rig_core::completion::{CompletionModel, CompletionResponse, Usage};
use rig_core::message::{AssistantContent, Message};
use rig_core::tool::{ToolOutcome, ToolResultExtensions};
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

    async fn on_completion_call(
        &self,
        _prompt: &Message,
        _history: &[Message],
        _turn: usize,
    ) -> Flow {
        Flow::cont()
    }

    async fn on_completion_response<M>(
        &self,
        _prompt: &Message,
        _response: &CompletionResponse<M::Response>,
    ) -> Flow
    where
        M: CompletionModel,
    {
        Flow::cont()
    }

    async fn on_model_turn_finished(
        &self,
        _turn: usize,
        _content: &OneOrMany<AssistantContent>,
        _usage: Usage,
    ) -> Flow {
        Flow::cont()
    }

    async fn on_invalid_tool_call(&self, _cx: &InvalidToolCallContext) -> Flow {
        Flow::cont()
    }

    async fn on_tool_call(
        &self,
        tool_name: &str,
        _tool_call_id: Option<&str>,
        internal_call_id: &str,
        args: &str,
    ) -> Flow {
        let call_at = time::Instant::now();

        let tool_input: Value =
            serde_json::from_str(args).unwrap_or_else(|_| Value::String(args.to_owned()));

        if tool_name == aries_tools::agent::NAME {
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
                Flow::cont()
            },
            HookDecision::Terminate { reason } => {
                let mut last_tool_call_at = self.last_tool_call_at.lock();
                *last_tool_call_at = None;
                Flow::terminate(reason)
            },
        }
    }

    async fn on_tool_result(
        &self,
        tool_name: &str,
        _tool_call_id: Option<&str>,
        internal_call_id: &str,
        args: &str,
        result: &str,
        _outcome: &ToolOutcome,
        _extensions: &ToolResultExtensions,
    ) -> Flow {
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
            return Flow::cont();
        }

        if tool_name == aries_tools::agent::NAME {
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

        Flow::cont()
    }

    async fn on_text_delta(&self, _delta: &str, _aggregated: &str) -> Flow {
        Flow::cont()
    }

    async fn on_tool_call_delta(
        &self,
        _tool_call_id: &str,
        _internal_call_id: &str,
        _tool_name: Option<&str>,
        _delta: &str,
    ) -> Flow {
        Flow::cont()
    }

    async fn on_stream_response_finish<M>(
        &self,
        _prompt: &Message,
        _response: &<M as CompletionModel>::StreamingResponse,
    ) -> Flow
    where
        M: CompletionModel,
    {
        Flow::cont()
    }
}

impl<M> AgentHook<M> for SessionPromptHook
where
    M: CompletionModel,
{
    async fn on_event(&self, _ctx: &HookContext, event: StepEvent<'_, M>) -> Flow {
        match event {
            StepEvent::CompletionCall { prompt, history, turn } => {
                self.on_completion_call(prompt, history, turn).await
            },
            StepEvent::CompletionResponse { prompt, response } => {
                self.on_completion_response::<M>(prompt, response).await
            },
            StepEvent::ModelTurnFinished { turn, content, usage } => {
                self.on_model_turn_finished(turn, content, usage).await
            },
            StepEvent::InvalidToolCall(cx) => self.on_invalid_tool_call(cx).await,
            StepEvent::ToolCall { tool_name, tool_call_id, internal_call_id, args } => {
                self.on_tool_call(tool_name, tool_call_id, internal_call_id, args).await
            },
            StepEvent::ToolResult {
                tool_name,
                tool_call_id,
                internal_call_id,
                args,
                result,
                outcome,
                extensions,
            } => {
                self.on_tool_result(
                    tool_name,
                    tool_call_id,
                    internal_call_id,
                    args,
                    result,
                    outcome,
                    extensions,
                )
                .await
            },
            StepEvent::TextDelta { delta, aggregated } => {
                self.on_text_delta(delta, aggregated).await
            },
            StepEvent::ToolCallDelta { tool_call_id, internal_call_id, tool_name, delta } => {
                self.on_tool_call_delta(tool_call_id, internal_call_id, tool_name, delta).await
            },
            StepEvent::StreamResponseFinish { prompt, response } => {
                self.on_stream_response_finish::<M>(prompt, response).await
            },
            _ => Flow::cont(),
        }
    }

    async fn resolve_tool_call(
        &self,
        ctx: &HookContext,
        tool_name: &str,
        tool_call_id: Option<&str>,
        internal_call_id: &str,
        args: &str,
    ) -> (Flow, Option<Value>) {
        let event = StepEvent::ToolCall { tool_name, tool_call_id, internal_call_id, args };

        let flow = <Self as AgentHook<M>>::on_event(self, ctx, event).await;
        (flow, None)
    }

    fn observes(&self, kind: StepEventKind) -> bool {
        let _ = kind;
        true
    }
}
