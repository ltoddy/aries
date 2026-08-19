use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time;

use aries_compact::{ContextWindow, TokenEstimator, micro_compact};
use aries_event::Notifier;
use aries_extension::hook::input::{
    PostToolUseFailureHookInput, PostToolUseHookInput, PreToolUseHookInput, SubagentStartHookInput,
    SubagentStopHookInput,
};
use aries_extension::hook::{HookDecision, HooksExecutor};
use aries_persistence::ToolCallRepository;
use rig::agent::hook::CompletionCall;
use rig::agent::{
    AgentHook, CompletionCallAction, HookContext, InvalidToolCallAction, InvalidToolCallContext,
    ModelTurnAction, ModelTurnFinished, ObservationAction, RequestPatch, StepEventKind,
    StreamResponseFinish, TextDelta, ToolCall, ToolCallAction, ToolCallDelta, ToolResultAction,
    ToolResultEvent,
};
use rig::message::Message;
use serde_json::Value;
use toasty::Db;
use tokio::sync::Mutex;

use crate::session::instruction::SharedInstructionContext;

const MICRO_COMPACT_KEEP_MESSAGES_AT_80_PERCENT: usize = 10;
const MICRO_COMPACT_KEEP_MESSAGES_AT_75_PERCENT: usize = 15;
const MICRO_COMPACT_KEEP_MESSAGES_AT_70_PERCENT: usize = 20;
const MICRO_COMPACT_KEEP_MESSAGES_AT_65_PERCENT: usize = 25;
const MICRO_COMPACT_KEEP_MESSAGES_AT_60_PERCENT: usize = 30;

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
    instruction_ctx: SharedInstructionContext,
    window: ContextWindow,

    notifier: Notifier,
}

impl SessionPromptHook {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        executor: Arc<HooksExecutor>,
        session_id: impl Into<String>,
        cwd: impl AsRef<Path>,
        transcript_path: impl AsRef<Path>,
        agent_id: impl Into<String>,
        agent_type: impl Into<String>,
        db: Db,
        notifier: Notifier,
    ) -> Self {
        let session_id = session_id.into();
        let cwd = cwd.as_ref();
        let transcript_path = transcript_path.as_ref().to_owned();
        let agent_id = agent_id.into();
        let agent_type = agent_type.into();

        let tool_call_repo = ToolCallRepository::new(db);
        let instruction_ctx = SharedInstructionContext::new(cwd);
        let window = ContextWindow::new();

        Self {
            executor,
            session_id,
            cwd: cwd.to_owned(),
            transcript_path,
            agent_id,
            agent_type,
            last_tool_call_at: Default::default(),
            tool_call_repo,
            instruction_ctx,
            window,
            notifier,
        }
    }
}

impl AgentHook for SessionPromptHook {
    async fn on_completion_call(
        &self,
        _ctx: &HookContext,
        event: CompletionCall<'_>,
    ) -> CompletionCallAction {
        let instructions = self.instruction_ctx.drain().await;

        let estimate_tokens = event.history.estimate_tokens() + event.prompt.estimate_tokens();
        let stuffed = estimate_tokens > self.window.sixty_percent_threshold();
        if instructions.is_empty() && !stuffed {
            return CompletionCallAction::continue_run();
        }

        let mut patched = event.history.to_vec();

        if estimate_tokens > self.window.eighty_percent_threshold() {
            micro_compact(&mut patched, MICRO_COMPACT_KEEP_MESSAGES_AT_80_PERCENT);
        } else if estimate_tokens > self.window.seventy_five_percent_threshold() {
            micro_compact(&mut patched, MICRO_COMPACT_KEEP_MESSAGES_AT_75_PERCENT);
        } else if estimate_tokens > self.window.seventy_percent_threshold() {
            micro_compact(&mut patched, MICRO_COMPACT_KEEP_MESSAGES_AT_70_PERCENT);
        } else if estimate_tokens > self.window.sixty_five_percent_threshold() {
            micro_compact(&mut patched, MICRO_COMPACT_KEEP_MESSAGES_AT_65_PERCENT);
        } else if estimate_tokens > self.window.sixty_percent_threshold() {
            micro_compact(&mut patched, MICRO_COMPACT_KEEP_MESSAGES_AT_60_PERCENT);
        }

        for instruction in instructions {
            let reminder = Message::user(
                ["<system-reminder>", &instruction.render(), "</system-reminder>"].join("\n"),
            );
            patched.push(reminder);
        }
        CompletionCallAction::Patch(RequestPatch::new().history(patched))
    }

    async fn on_completion_response(
        &self,
        _ctx: &HookContext,
        _event: rig::agent::hook::CompletionResponse<'_>,
    ) -> ObservationAction {
        ObservationAction::continue_run()
    }

    async fn on_model_turn_finished(
        &self,
        _ctx: &HookContext,
        _event: ModelTurnFinished<'_>,
    ) -> ModelTurnAction {
        ModelTurnAction::continue_run()
    }

    async fn on_invalid_tool_call(
        &self,
        _ctx: &HookContext,
        _event: &InvalidToolCallContext,
    ) -> Option<InvalidToolCallAction> {
        None
    }

    async fn on_tool_call(&self, _ctx: &HookContext, event: ToolCall<'_>) -> ToolCallAction {
        let mut last_tool_call_at = self.last_tool_call_at.lock().await;
        *last_tool_call_at = Some(time::Instant::now());

        let tool_input: Value = serde_json::from_str(event.args)
            .unwrap_or_else(|_| Value::String(event.args.to_owned()));

        match event.tool_name {
            aries_tools::agent::NAME => {
                let input = SubagentStartHookInput::new(
                    &self.session_id,
                    &self.cwd,
                    &self.agent_id,
                    &self.agent_type,
                )
                .transcript_path(&self.transcript_path);
                self.executor.fire_subagent_start(input).await;
            },
            aries_tools::read::NAME => {
                if let Ok(args) =
                    serde_json::from_value::<aries_tools::read::ReadArgs>(tool_input.clone())
                {
                    let file_path = args.file_path;
                    if let Some(parent) = file_path.parent() {
                        self.instruction_ctx.visit(parent).await;
                    }
                }
            },
            _ => {},
        }

        // AskUserQuestion is the only interactive tool today: suspend the turn
        // before execution and forward the model-generated arguments as the
        // question definition. Generalize into a registry if more interactive
        // tools appear.
        if event.tool_name == aries_tools::question::NAME {
            self.notifier.send_awaiting_input(tool_input.clone());
            return ToolCallAction::Stop(aries_agent::AWAITING_USER_INPUT_REASON.to_string());
        }

        let input = PreToolUseHookInput::new(
            &self.session_id,
            &self.cwd,
            event.tool_name,
            tool_input,
            event.internal_call_id,
        )
        .transcript_path(&self.transcript_path)
        .agent_id(&self.agent_id)
        .agent_type(&self.agent_type);

        match self.executor.fire_pre_tool_use(input).await {
            HookDecision::Continue => ToolCallAction::run(),
            HookDecision::Terminate { reason } => ToolCallAction::Stop(reason),
        }
    }

    async fn on_tool_result(
        &self,
        _ctx: &HookContext,
        event: ToolResultEvent<'_>,
    ) -> ToolResultAction {
        let duration_ms = self
            .last_tool_call_at
            .lock()
            .await
            .take()
            .map(|started_at| started_at.elapsed().as_millis() as u64);

        let tool_input: Value = serde_json::from_str(event.args)
            .unwrap_or_else(|_| Value::String(event.args.to_owned()));

        let was_successful = event.raw_result.is_success();
        let mut repo = self.tool_call_repo.clone();
        let _ = repo
            .create(
                &self.session_id,
                event.internal_call_id,
                event.tool_name,
                event.args,
                duration_ms,
                was_successful,
            )
            .await;

        if let Some(error) = event.raw_result.error() {
            let input = PostToolUseFailureHookInput::new(
                &self.session_id,
                &self.cwd,
                event.tool_name,
                &tool_input,
                event.internal_call_id,
                error.message(),
            )
            .transcript_path(&self.transcript_path)
            .is_interrupt(false);
            let input = match duration_ms {
                Some(duration_ms) => input.duration_ms(duration_ms),
                None => input,
            };

            self.executor.fire_post_tool_use_failure(input).await;
            return ToolResultAction::keep();
        }

        if event.tool_name == aries_tools::agent::NAME {
            let input = SubagentStopHookInput::new(
                &self.session_id,
                &self.cwd,
                false,
                &self.agent_id,
                &self.agent_type,
            )
            .transcript_path(&self.transcript_path);

            let input = match event.raw_result.output().as_text() {
                Some(text) => input.last_assistant_message(text),
                None => input,
            };
            self.executor.fire_subagent_stop(input).await;
        }

        let input = PostToolUseHookInput::new(
            &self.session_id,
            &self.cwd,
            event.tool_name,
            &tool_input,
            event.raw_result.output().as_json().unwrap_or_default(),
            event.internal_call_id,
        )
        .transcript_path(&self.transcript_path)
        .agent_id(&self.agent_id)
        .agent_type(&self.agent_type);
        let input = match duration_ms {
            Some(duration_ms) => input.duration_ms(duration_ms),
            None => input,
        };

        self.executor.fire_post_tool_use(input).await;

        ToolResultAction::keep()
    }

    async fn on_text_delta(&self, _ctx: &HookContext, _event: TextDelta<'_>) -> ObservationAction {
        ObservationAction::continue_run()
    }

    async fn on_tool_call_delta(
        &self,
        _ctx: &HookContext,
        _event: ToolCallDelta<'_>,
    ) -> ObservationAction {
        ObservationAction::continue_run()
    }

    async fn on_stream_response_finish(
        &self,
        _ctx: &HookContext,
        _event: StreamResponseFinish<'_>,
    ) -> ObservationAction {
        ObservationAction::continue_run()
    }

    fn observes(&self, _kind: StepEventKind) -> bool {
        true
    }
}
