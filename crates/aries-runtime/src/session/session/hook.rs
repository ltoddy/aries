use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time;

use aries_communicate::Notifier;
use aries_extension::hook::input::{
    PostToolUseFailureHookInput, PostToolUseHookInput, PreToolUseHookInput, SubagentStartHookInput,
    SubagentStopHookInput,
};
use aries_extension::hook::{HookDecision, HooksExecutor};
use rig::agent::{
    AgentHook, CompletionCall, CompletionCallAction, CompletionCallEvent, DispatchAction,
    DispatchEvent, HookContext, InvalidToolCallAction, InvalidToolCallContext, ModelSelection,
    ModelSelectionAction, ModelTurnAction, ModelTurnFinished, MultiTurnStreamItem,
    ObservationAction, OutcomeAction, OutcomeEvent, ReasoningDelta, RequestPatch, RunSettled,
    RunStart, RunStartAction, StepEventKind, TextDelta, ToolCallDelta,
};
use rig::effect::{Answered, EffectKind};
use rig::message::CallId;
use serde::Deserialize;
use serde_json::Value;
use tokio::sync::Mutex;
use tracing::warn;

use crate::compact::{ContextWindow, TokenEstimator, micro_compact};
use crate::session::session::instruction::InstructionContext;
use crate::session::session::{system_reminder, system_reminders};
use crate::tools::agent::AgentArgs;
use crate::tools::read::ReadArgs;

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
    tool_call_started_at: Arc<Mutex<HashMap<CallId, time::Instant>>>,
    instruction_ctx: InstructionContext,
    window: ContextWindow,
    notifier: Notifier,
}

impl SessionPromptHook {
    pub fn new(
        executor: Arc<HooksExecutor>,
        session_id: impl Into<String>,
        cwd: impl Into<PathBuf>,
        transcript_path: impl Into<PathBuf>,
        agent_id: impl Into<String>,
        instruction_ctx: InstructionContext,
        notifier: Notifier,
    ) -> Self {
        let session_id = session_id.into();
        let cwd = cwd.into();
        let transcript_path = transcript_path.into();
        let agent_id = agent_id.into();

        let window = ContextWindow::new();

        Self {
            executor,
            session_id,
            cwd,
            transcript_path,
            agent_id,
            tool_call_started_at: Arc::new(Mutex::new(HashMap::new())),
            instruction_ctx,
            window,
            notifier,
        }
    }

    async fn fire_post_tool_use_failure(
        &self,
        name: &str,
        args: &str,
        call_id: impl Into<String>,
        duration_ms: Option<u64>,
        error_message: &str,
    ) {
        let tool_input: Value =
            serde_json::from_str(args).unwrap_or_else(|_| Value::String(args.to_owned()));
        let input = PostToolUseFailureHookInput::new(
            &self.session_id,
            &self.cwd,
            name,
            &tool_input,
            call_id,
            error_message,
        )
        .transcript_path(&self.transcript_path)
        .is_interrupt(false);
        let input = match duration_ms {
            Some(duration_ms) => input.duration_ms(duration_ms),
            None => input,
        };

        if let HookDecision::Continue { contexts } =
            self.executor.fire_post_tool_use_failure(input).await
        {
            self.instruction_ctx.push_hook_contexts(contexts).await;
        }
    }
}

impl AgentHook for SessionPromptHook {
    fn name(&self) -> Option<String> {
        Some(String::from("AgentHook"))
    }

    async fn on_run_start(&self, _ctx: &HookContext, _event: RunStart<'_>) -> RunStartAction {
        RunStartAction::continue_run()
    }

    async fn on_run_settled(&self, _ctx: &HookContext, _event: RunSettled<'_>) {}

    fn on_model_select(
        &self,
        _ctx: &HookContext,
        _event: ModelSelection<'_>,
    ) -> ModelSelectionAction {
        ModelSelectionAction::continue_run()
    }

    async fn on_completion_call(
        &self,
        _ctx: &HookContext,
        event: CompletionCallEvent<'_>,
    ) -> CompletionCallAction {
        let instructions = self.instruction_ctx.drain().await;
        let hook_contexts = self.instruction_ctx.drain_hook_contexts().await;

        let estimate_tokens = event.history.estimate_tokens() + event.prompt.estimate_tokens();
        let stuffed = estimate_tokens > self.window.sixty_percent_threshold();
        if instructions.is_empty() && hook_contexts.is_empty() && !stuffed {
            return CompletionCallAction::continue_run();
        }

        let mut patched: Vec<_> =
            event.history.iter().map(|message| message.as_ref().clone()).collect();

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
            let reminder = system_reminder(instruction.render());
            patched.push(reminder);
        }
        patched.extend(system_reminders(hook_contexts));
        CompletionCallAction::patch(RequestPatch::new().history(patched))
    }

    async fn on_model_turn_finished(
        &self,
        _ctx: &HookContext,
        event: ModelTurnFinished<'_>,
    ) -> ModelTurnAction {
        self.notifier.send_stream_item(MultiTurnStreamItem::CompletionCall(CompletionCall::new(
            event.turn,
            event.usage,
            event.raw.clone(),
        )));
        ModelTurnAction::continue_run()
    }

    async fn on_invalid_tool_call(
        &self,
        _ctx: &HookContext,
        _event: &InvalidToolCallContext,
    ) -> Option<InvalidToolCallAction> {
        None
    }

    async fn on_text_delta(&self, _ctx: &HookContext, _event: TextDelta<'_>) -> ObservationAction {
        ObservationAction::continue_run()
    }

    async fn on_reasoning_delta(
        &self,
        _ctx: &HookContext,
        _event: ReasoningDelta<'_>,
    ) -> ObservationAction {
        ObservationAction::continue_run()
    }

    async fn on_tool_call_delta(
        &self,
        _ctx: &HookContext,
        _event: ToolCallDelta<'_>,
    ) -> ObservationAction {
        ObservationAction::continue_run()
    }

    async fn on_dispatch(&self, _ctx: &HookContext, event: DispatchEvent<'_>) -> DispatchAction {
        let EffectKind::ToolCall { name, args } = event.kind else {
            return DispatchAction::proceed();
        };
        let Some(call_id) = event.call_id else {
            return DispatchAction::proceed();
        };

        self.tool_call_started_at.lock().await.insert(call_id.clone(), time::Instant::now());

        let mut tool_input: Value =
            serde_json::from_str(args).unwrap_or_else(|_| Value::String(args.clone()));
        let mut patched_tool_input = false;

        match name.as_str() {
            crate::tools::agent::NAME => {
                let input =
                    SubagentStartHookInput::new(&self.session_id, &self.cwd, &self.agent_id)
                        .transcript_path(&self.transcript_path);

                match self.executor.fire_subagent_start(input).await {
                    HookDecision::Continue { contexts } => {
                        let contexts: Vec<_> = contexts.into_iter().collect();
                        if !contexts.is_empty() {
                            let Ok(mut args) = AgentArgs::deserialize(&tool_input) else {
                                warn!("failed to parse Agent args for SubagentStart context");
                                return DispatchAction::proceed();
                            };

                            args.prompt.push_str("\n\n");
                            args.prompt.push_str(&render_contexts(contexts));
                            match serde_json::to_value(args) {
                                Ok(patched) => tool_input = patched,
                                Err(err) => {
                                    warn!(%err, "failed to serialize Agent args for SubagentStart context");
                                },
                            }
                            patched_tool_input = true;
                        }
                    },
                    HookDecision::Terminate { reason } => return DispatchAction::stop(reason),
                }
            },
            crate::tools::read::NAME => {
                if let Ok(args) = ReadArgs::deserialize(&tool_input) {
                    let file_path = args.file_path;
                    if let Some(parent) = file_path.parent() {
                        self.instruction_ctx.visit(parent).await;
                    }
                }
            },
            _ => {},
        }

        let input = PreToolUseHookInput::new(
            &self.session_id,
            &self.cwd,
            name,
            &tool_input,
            call_id.to_string(),
        )
        .transcript_path(&self.transcript_path)
        .agent_id(&self.agent_id);

        match self.executor.fire_pre_tool_use(input).await {
            HookDecision::Continue { contexts } => {
                self.instruction_ctx.push_hook_contexts(contexts).await;
                if patched_tool_input {
                    DispatchAction::rewrite_tool_args(event.kind, tool_input)
                } else {
                    DispatchAction::proceed()
                }
            },
            HookDecision::Terminate { reason } => DispatchAction::stop(reason),
        }
    }

    async fn on_outcome(&self, _ctx: &HookContext, event: OutcomeEvent<'_>) -> OutcomeAction {
        match (event.kind, event.outcome, event.call_id) {
            (
                EffectKind::ToolCall { name, args },
                Ok(Answered::ToolResult { result }),
                Some(call_id),
            ) => {
                let duration_ms = self
                    .tool_call_started_at
                    .lock()
                    .await
                    .remove(call_id)
                    .map(|started_at| started_at.elapsed().as_millis() as u64);

                let tool_output = result.output();
                let tool_input: Value =
                    serde_json::from_str(args).unwrap_or_else(|_| Value::String(args.clone()));

                if let Some(error) = result.error() {
                    self.fire_post_tool_use_failure(
                        name,
                        args,
                        call_id.to_string(),
                        duration_ms,
                        error.message(),
                    )
                    .await;
                    return OutcomeAction::proceed();
                }

                if name == crate::tools::agent::NAME {
                    let input = SubagentStopHookInput::new(
                        &self.session_id,
                        &self.cwd,
                        false,
                        &self.agent_id,
                    )
                    .transcript_path(&self.transcript_path);

                    let input = match tool_output.as_text() {
                        Some(text) => input.last_assistant_message(text),
                        None => input,
                    };
                    if let HookDecision::Continue { contexts } =
                        self.executor.fire_subagent_stop(input).await
                    {
                        self.instruction_ctx.push_hook_contexts(contexts).await;
                    }
                }

                let input = PostToolUseHookInput::new(
                    &self.session_id,
                    &self.cwd,
                    name,
                    &tool_input,
                    tool_output.as_json().unwrap_or_default(),
                    call_id.to_string(),
                )
                .transcript_path(&self.transcript_path)
                .agent_id(&self.agent_id);
                let input = match duration_ms {
                    Some(duration_ms) => input.duration_ms(duration_ms),
                    None => input,
                };

                if let HookDecision::Continue { contexts } =
                    self.executor.fire_post_tool_use(input).await
                {
                    self.instruction_ctx.push_hook_contexts(contexts).await;
                }

                OutcomeAction::proceed()
            },
            (EffectKind::ToolCall { name, args }, Err(err), Some(call_id)) => {
                let duration_ms = self
                    .tool_call_started_at
                    .lock()
                    .await
                    .remove(call_id)
                    .map(|started_at| started_at.elapsed().as_millis() as u64);

                self.fire_post_tool_use_failure(
                    name,
                    args,
                    call_id.to_string(),
                    duration_ms,
                    &err.message,
                )
                .await;
                OutcomeAction::proceed()
            },
            (_, _, _) => OutcomeAction::proceed(),
        }
    }

    fn observes(&self, _kind: StepEventKind) -> bool {
        true
    }
}

fn render_contexts<T: AsRef<str>>(contexts: impl IntoIterator<Item = T>) -> String {
    let mut rendered = Vec::new();
    for context in contexts {
        rendered.push(["<system-reminder>", context.as_ref(), "</system-reminder>"].join("\n"));
    }
    rendered.join("\n\n")
}
