mod command;
mod decision;
mod http;
mod output;
#[cfg(test)]
mod tests;

use std::collections::{HashMap, VecDeque};
use std::fmt::Debug;

use serde::Serialize;
use tracing::{info, warn};

use self::command::{BashHookOutput, execute_bash_command_hook};
pub use self::decision::HookDecision;
use self::http::execute_http_hook;
use crate::hook::definition::{HookCommand, HookEvent, HookMatcher, HooksDefinition};
use crate::hook::executor::http::HttpHookOutcome;
use crate::hook::input::{
    HookInput, PostCompactHookInput, PostToolUseFailureHookInput, PostToolUseHookInput,
    PreCompactHookInput, PreToolUseHookInput, SessionEndHookInput, SessionStartHookInput,
    StopFailureHookInput, StopHookInput, SubagentStartHookInput, SubagentStopHookInput,
    UserPromptSubmitHookInput,
};

#[derive(Debug)]
pub struct HooksExecutor {
    hooks: HashMap<HookEvent, Vec<HookMatcher>>,
}

impl HooksExecutor {
    pub fn new(presets: Vec<HooksDefinition>) -> Self {
        let mut hooks = HashMap::<HookEvent, Vec<HookMatcher>>::new();

        for preset in presets {
            let settings = preset.hooks.0;
            hooks.extend(settings);
        }

        Self { hooks }
    }

    pub async fn fire_post_compact(&self, input: PostCompactHookInput) {
        let hook_event_name = input.hook_event_name();
        let input = match serde_json::to_string(&input) {
            Ok(input) => input,
            Err(err) => {
                warn!(event = hook_event_name, %err, "failed to serialize hook input");
                return;
            },
        };
        info!(event = hook_event_name, input = %input, "received hook event");

        let Some(matchers) = self.hooks.get(&HookEvent::PostCompact) else { return };

        for matcher in matchers {
            if let HookDecision::Terminate { reason } =
                Self::fire_hooks(&matcher.hooks, input.clone(), hook_event_name).await
            {
                warn!(event = hook_event_name, %reason, "hook exited with code 2");
            }
        }
    }

    pub async fn fire_post_tool_use_failure<ToolInput>(
        &self,
        input: PostToolUseFailureHookInput<ToolInput>,
    ) -> HookDecision
    where
        ToolInput: Serialize + Clone + Debug,
    {
        let hook_event_name = input.hook_event_name();
        let tool_name = input.tool_name.as_str();
        let input = match serde_json::to_string(&input) {
            Ok(input) => input,
            Err(err) => {
                warn!(event = hook_event_name, %err, "failed to serialize hook input");
                return HookDecision::r#continue([]);
            },
        };
        info!(event = hook_event_name, input = %input, "received hook event");

        let Some(matchers) = self.hooks.get(&HookEvent::PostToolUseFailure) else {
            return HookDecision::r#continue([]);
        };

        let mut contexts = VecDeque::new();
        for matcher in matchers {
            match matcher.matches(tool_name) {
                Ok(true) => {
                    match Self::fire_hooks(&matcher.hooks, input.clone(), hook_event_name).await {
                        HookDecision::Terminate { reason } => {
                            warn!(event = hook_event_name, %reason, "hook exited with code 2");
                            return HookDecision::terminate(reason);
                        },
                        HookDecision::Continue { contexts: next_contexts } => {
                            contexts.extend(next_contexts)
                        },
                    }
                },
                Err(err) => warn!("invalid hook matcher, skipped: {err}"),
                _ => {},
            }
        }

        HookDecision::r#continue(contexts)
    }

    pub async fn fire_post_tool_use<ToolInput, ToolResponse>(
        &self,
        input: PostToolUseHookInput<ToolInput, ToolResponse>,
    ) -> HookDecision
    where
        ToolInput: Clone + Debug + Default + Serialize,
        ToolResponse: Clone + Debug + Default + Serialize,
    {
        let hook_event_name = input.hook_event_name();
        let tool_name = input.tool_name.as_str();
        let input = match serde_json::to_string(&input) {
            Ok(input) => input,
            Err(err) => {
                warn!(event = hook_event_name, %err, "failed to serialize hook input");
                return HookDecision::r#continue([]);
            },
        };
        info!(event = hook_event_name, input = %input, "received hook event");

        let Some(matchers) = self.hooks.get(&HookEvent::PostToolUse) else {
            return HookDecision::r#continue([]);
        };
        let mut contexts = VecDeque::new();

        for matcher in matchers {
            match matcher.matches(tool_name) {
                Ok(true) => {
                    match Self::fire_hooks(&matcher.hooks, input.clone(), hook_event_name).await {
                        HookDecision::Terminate { reason } => {
                            warn!(event = hook_event_name, %reason, "hook exited with code 2");
                            return HookDecision::terminate(reason);
                        },
                        HookDecision::Continue { contexts: next_contexts } => {
                            contexts.extend(next_contexts)
                        },
                    }
                },
                Err(err) => warn!("invalid hook matcher, skipped: {err}"),
                _ => {},
            }
        }

        HookDecision::r#continue(contexts)
    }

    pub async fn fire_pre_compact(&self, input: PreCompactHookInput) -> HookDecision {
        let hook_event_name = input.hook_event_name();
        let input = match serde_json::to_string(&input) {
            Ok(input) => input,
            Err(err) => {
                warn!(event = hook_event_name, %err, "failed to serialize hook input");
                return HookDecision::r#continue([]);
            },
        };
        info!(event = hook_event_name, input = %input, "received hook event");

        let Some(matchers) = self.hooks.get(&HookEvent::PreCompact) else {
            return HookDecision::r#continue([]);
        };

        for matcher in matchers {
            if let HookDecision::Terminate { reason } =
                Self::fire_hooks(&matcher.hooks, input.clone(), hook_event_name).await
            {
                warn!(event = hook_event_name, %reason, "hook blocked with exit code 2");
                return HookDecision::terminate(reason);
            }
        }

        HookDecision::r#continue([])
    }

    pub async fn fire_pre_tool_use<ToolInput>(
        &self,
        input: PreToolUseHookInput<ToolInput>,
    ) -> HookDecision
    where
        ToolInput: Clone + Debug + Default + Serialize,
    {
        let hook_event_name = input.hook_event_name();
        let tool_name = input.tool_name.clone();
        let input = match serde_json::to_string(&input) {
            Ok(input) => input,
            Err(err) => {
                warn!(event = hook_event_name, %err, "failed to serialize hook input");
                return HookDecision::r#continue([]);
            },
        };
        info!(event = hook_event_name, input = %input, "received hook event");
        let Some(matchers) = self.hooks.get(&HookEvent::PreToolUse) else {
            return HookDecision::r#continue([]);
        };

        let mut contexts = VecDeque::new();
        for matcher in matchers {
            match matcher.matches(&tool_name) {
                Ok(true) => {
                    match Self::fire_hooks(&matcher.hooks, input.clone(), hook_event_name).await {
                        HookDecision::Terminate { reason } => {
                            warn!(event = hook_event_name, %reason, "hook blocked with exit code 2");
                            return HookDecision::terminate(reason);
                        },
                        HookDecision::Continue { contexts: next_contexts } => {
                            contexts.extend(next_contexts)
                        },
                    }
                },
                Err(err) => warn!("invalid hook matcher, skipped: {err}"),
                _ => {},
            }
        }

        HookDecision::r#continue(contexts)
    }

    pub async fn fire_session_end(&self, input: SessionEndHookInput) {
        let hook_event_name = input.hook_event_name();
        let input = match serde_json::to_string(&input) {
            Ok(input) => input,
            Err(err) => {
                warn!(event = hook_event_name, %err, "failed to serialize hook input");
                return;
            },
        };
        info!(event = hook_event_name, input = %input, "received hook event");

        let Some(matchers) = self.hooks.get(&HookEvent::SessionEnd) else { return };

        for matcher in matchers {
            if let HookDecision::Terminate { reason } =
                Self::fire_hooks(&matcher.hooks, input.clone(), hook_event_name).await
            {
                warn!(event = hook_event_name, %reason, "hook exited with code 2");
            }
        }
    }

    pub async fn fire_session_start(&self, input: SessionStartHookInput) -> HookDecision {
        let hook_event_name = input.hook_event_name();
        let input = match serde_json::to_string(&input) {
            Ok(input) => input,
            Err(err) => {
                warn!(event = hook_event_name, %err, "failed to serialize hook input");
                return HookDecision::r#continue([]);
            },
        };
        info!(event = hook_event_name, input = %input, "received hook event");

        let Some(matchers) = self.hooks.get(&HookEvent::SessionStart) else {
            return HookDecision::r#continue([]);
        };
        let mut contexts = VecDeque::new();

        for matcher in matchers {
            match Self::fire_hooks(&matcher.hooks, input.clone(), hook_event_name).await {
                HookDecision::Terminate { reason } => {
                    warn!(event = hook_event_name, %reason, "hook exited with code 2");
                },
                HookDecision::Continue { contexts: next_contexts } => {
                    contexts.extend(next_contexts)
                },
            }
        }

        HookDecision::r#continue(contexts)
    }

    pub async fn fire_stop_failure(&self, input: StopFailureHookInput) {
        let hook_event_name = input.hook_event_name();
        let input = match serde_json::to_string(&input) {
            Ok(input) => input,
            Err(err) => {
                warn!(event = hook_event_name, %err, "failed to serialize hook input");
                return;
            },
        };
        info!(event = hook_event_name, input = %input, "received hook event");

        let Some(matchers) = self.hooks.get(&HookEvent::StopFailure) else { return };

        for matcher in matchers {
            if let HookDecision::Terminate { reason } =
                Self::fire_hooks(&matcher.hooks, input.clone(), hook_event_name).await
            {
                warn!(event = hook_event_name, %reason, "hook exited with code 2");
            }
        }
    }

    /// 在文档中对于 stop hook 是可以被阻止的: `Prevents the agent from stopping, continues the conversation`. 没太理解, 先记录个 todo
    pub async fn fire_stop(&self, input: StopHookInput) -> HookDecision {
        let hook_event_name = input.hook_event_name();
        let input = match serde_json::to_string(&input) {
            Ok(input) => input,
            Err(err) => {
                warn!(event = hook_event_name, %err, "failed to serialize hook input");
                return HookDecision::r#continue([]);
            },
        };
        info!(event = hook_event_name, input = %input, "received hook event");

        let Some(matchers) = self.hooks.get(&HookEvent::Stop) else {
            return HookDecision::r#continue([]);
        };

        let mut contexts = VecDeque::new();
        for matcher in matchers {
            match Self::fire_hooks(&matcher.hooks, input.clone(), hook_event_name).await {
                HookDecision::Terminate { reason } => {
                    warn!(event = hook_event_name, %reason, "hook exited with code 2");
                    return HookDecision::terminate(reason);
                },
                HookDecision::Continue { contexts: next_contexts } => {
                    contexts.extend(next_contexts)
                },
            }
        }

        HookDecision::r#continue(contexts)
    }

    pub async fn fire_subagent_start(&self, input: SubagentStartHookInput) -> HookDecision {
        let hook_event_name = input.hook_event_name();
        let input = match serde_json::to_string(&input) {
            Ok(input) => input,
            Err(err) => {
                warn!(event = hook_event_name, %err, "failed to serialize hook input");
                return HookDecision::r#continue([]);
            },
        };
        info!(event = hook_event_name, input = %input, "received hook event");

        let Some(matchers) = self.hooks.get(&HookEvent::SubagentStart) else {
            return HookDecision::r#continue([]);
        };
        let mut contexts = VecDeque::new();

        for matcher in matchers {
            match Self::fire_hooks(&matcher.hooks, input.clone(), hook_event_name).await {
                HookDecision::Terminate { reason } => {
                    warn!(event = hook_event_name, %reason, "hook exited with code 2");
                },
                HookDecision::Continue { contexts: next_contexts } => {
                    contexts.extend(next_contexts)
                },
            }
        }

        HookDecision::r#continue(contexts)
    }

    /// 在文档中对于 subagent stop hook 是可以被阻止的: `Prevents the subagent from stopping` 没太理解, 先记录一个 todo
    pub async fn fire_subagent_stop(&self, input: SubagentStopHookInput) -> HookDecision {
        let hook_event_name = input.hook_event_name();
        let input = match serde_json::to_string(&input) {
            Ok(input) => input,
            Err(err) => {
                warn!(event = hook_event_name, %err, "failed to serialize hook input");
                return HookDecision::r#continue([]);
            },
        };
        info!(event = hook_event_name, input = %input, "received hook event");

        let Some(matchers) = self.hooks.get(&HookEvent::SubagentStop) else {
            return HookDecision::r#continue([]);
        };

        let mut contexts = VecDeque::new();
        for matcher in matchers {
            match Self::fire_hooks(&matcher.hooks, input.clone(), hook_event_name).await {
                HookDecision::Terminate { reason } => {
                    warn!(event = hook_event_name, %reason, "hook exited with code 2");
                    return HookDecision::terminate(reason);
                },
                HookDecision::Continue { contexts: next_contexts } => {
                    contexts.extend(next_contexts)
                },
            }
        }

        HookDecision::r#continue(contexts)
    }

    pub async fn fire_user_prompt_submit(&self, input: UserPromptSubmitHookInput) -> HookDecision {
        let hook_event_name = input.hook_event_name();
        let input = match serde_json::to_string(&input) {
            Ok(input) => input,
            Err(err) => {
                warn!(event = hook_event_name, %err, "failed to serialize hook input");
                return HookDecision::r#continue([]);
            },
        };
        info!(event = hook_event_name, input = %input, "received hook event");

        let Some(matchers) = self.hooks.get(&HookEvent::UserPromptSubmit) else {
            return HookDecision::r#continue([]);
        };

        let mut contexts = VecDeque::new();
        for matcher in matchers {
            match Self::fire_hooks(&matcher.hooks, input.clone(), hook_event_name).await {
                HookDecision::Terminate { reason } => {
                    warn!(event = hook_event_name, %reason, "hook exited with code 2");
                    return HookDecision::terminate(reason);
                },
                HookDecision::Continue { contexts: next_contexts } => {
                    contexts.extend(next_contexts)
                },
            }
        }

        HookDecision::r#continue(contexts)
    }

    async fn fire_hooks(
        hooks: &[HookCommand],
        input: impl Into<String>,
        hook_event_name: &str,
    ) -> HookDecision {
        let payload = input.into();

        for hook in hooks {
            match hook {
                HookCommand::Command(bash) => {
                    match execute_bash_command_hook(bash, &payload).await {
                        Ok(outcome) => match outcome.output(hook_event_name) {
                            BashHookOutput::Terminate { reason } => {
                                return HookDecision::terminate(reason);
                            },
                            BashHookOutput::Continue { context: Some(context) } => {
                                return HookDecision::r#continue([context]);
                            },
                            BashHookOutput::Continue { context: None } => {},
                        },
                        Err(err) => {
                            let reason = format!("bash hook execute failed: {err}");
                            return HookDecision::terminate(reason);
                        },
                    }
                },
                HookCommand::Prompt(_) => {
                    // TODO 未来可能会移除这个分支
                },
                HookCommand::Http(http) => match execute_http_hook(http, &payload).await {
                    Ok(response) => match response {
                        HttpHookOutcome::Json(json) => {
                            if !json.should_continue {
                                let reason = json.stop_reason.unwrap_or_else(|| {
                                    format!("{hook_event_name} http hook returned continue: false")
                                });
                                return HookDecision::terminate(reason);
                            }
                            if let Some(context) = json.additional_context(hook_event_name) {
                                return HookDecision::r#continue([context]);
                            }
                        },
                        HttpHookOutcome::Text(text) => {
                            info!(event = hook_event_name, %text, "http hook receive plain text");
                            return HookDecision::r#continue([text]);
                        },
                        HttpHookOutcome::Empty => {},
                    },
                    Err(err) => {
                        warn!(event = hook_event_name, %err, "http hook failed, execution continues")
                    },
                },
            }
        }

        HookDecision::r#continue([])
    }
}
