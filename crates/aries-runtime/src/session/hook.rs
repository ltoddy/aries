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
    DispatchEvent, HookContext, ModelTurnAction, ModelTurnFinished, MultiTurnStreamItem,
    OutcomeAction, OutcomeEvent, RequestPatch,
};
use rig::effect::{Answered, EffectKind};
use rig::message::CallId;
use serde::Deserialize;
use serde_json::Value;
use tokio::sync::Mutex;
use tracing::warn;

use crate::context::compaction::{ContextWindow, TokenEstimator, micro_compact};
use crate::state::State;
use crate::tools::agent::AgentArgs;

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
    transcript_dir: PathBuf,
    agent_id: String,
    tool_call_started_at: Arc<Mutex<HashMap<CallId, time::Instant>>>,
    window: ContextWindow,
    notifier: Notifier,
}

impl SessionPromptHook {
    pub fn new(
        state: State,
        executor: Arc<HooksExecutor>,
        session_id: impl Into<String>,
        cwd: impl Into<PathBuf>,
        transcript_dir: impl Into<PathBuf>,
        agent_id: impl Into<String>,
        notifier: Notifier,
    ) -> Self {
        let session_id = session_id.into();
        let cwd = cwd.into();
        let transcript_dir = transcript_dir.into();
        let agent_id = agent_id.into();

        let model = state.model();
        let vendor = model.wire.provider().dialect.name;
        let model = model.name();
        let window = ContextWindow::new(vendor, model);

        Self {
            executor,
            session_id,
            cwd,
            transcript_dir,
            agent_id,
            tool_call_started_at: Arc::new(Mutex::new(HashMap::new())),
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
        .transcript_dir(&self.transcript_dir)
        .is_interrupt(false);
        let input = match duration_ms {
            Some(duration_ms) => input.duration_ms(duration_ms),
            None => input,
        };

        let _ = self.executor.fire_post_tool_use_failure(input).await;
    }
}

impl AgentHook for SessionPromptHook {
    fn name(&self) -> Option<String> {
        Some(String::from("AgentHook"))
    }

    async fn on_completion_call(
        &self,
        _ctx: &HookContext,
        event: CompletionCallEvent<'_>,
    ) -> CompletionCallAction {
        let estimate_tokens = event.history.estimate_tokens() + event.prompt.estimate_tokens();
        let stuffed = estimate_tokens > self.window.sixty_percent_threshold();
        if !stuffed {
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

        if name == crate::tools::agent::NAME {
            let input = SubagentStartHookInput::new(&self.session_id, &self.cwd, &self.agent_id)
                .transcript_dir(&self.transcript_dir);

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
        }

        let input = PreToolUseHookInput::new(
            &self.session_id,
            &self.cwd,
            name,
            &tool_input,
            call_id.to_string(),
        )
        .transcript_dir(&self.transcript_dir)
        .agent_id(&self.agent_id);

        match self.executor.fire_pre_tool_use(input).await {
            HookDecision::Continue { .. } => {
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
                    .transcript_dir(&self.transcript_dir);

                    let input = match tool_output.as_text() {
                        Some(text) => input.last_assistant_message(text),
                        None => input,
                    };
                    let _ = self.executor.fire_subagent_stop(input).await;
                }

                let input = PostToolUseHookInput::new(
                    &self.session_id,
                    &self.cwd,
                    name,
                    &tool_input,
                    tool_output.as_json().unwrap_or_default(),
                    call_id.to_string(),
                )
                .transcript_dir(&self.transcript_dir)
                .agent_id(&self.agent_id);
                let input = match duration_ms {
                    Some(duration_ms) => input.duration_ms(duration_ms),
                    None => input,
                };

                let _ = self.executor.fire_post_tool_use(input).await;

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
}

fn render_contexts<T: AsRef<str>>(contexts: impl IntoIterator<Item = T>) -> String {
    let mut rendered = Vec::new();
    for context in contexts {
        rendered.push(["<system-reminder>", context.as_ref(), "</system-reminder>"].join("\n"));
    }
    rendered.join("\n\n")
}
