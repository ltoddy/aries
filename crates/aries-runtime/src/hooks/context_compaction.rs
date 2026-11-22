use rig::agent::{AgentHook, CompletionCallAction, CompletionCallEvent, HookContext, RequestPatch};

use crate::context::compaction::{ContextWindow, TokenEstimator, micro_compact};

const MICRO_COMPACT_KEEP_MESSAGES_AT_80_PERCENT: usize = 10;
const MICRO_COMPACT_KEEP_MESSAGES_AT_75_PERCENT: usize = 15;
const MICRO_COMPACT_KEEP_MESSAGES_AT_70_PERCENT: usize = 20;
const MICRO_COMPACT_KEEP_MESSAGES_AT_65_PERCENT: usize = 25;
const MICRO_COMPACT_KEEP_MESSAGES_AT_60_PERCENT: usize = 30;

#[derive(Debug, Clone)]
pub struct ContextCompactionHook {
    window: ContextWindow,
}

impl Default for ContextCompactionHook {
    fn default() -> Self {
        Self::new()
    }
}

impl ContextCompactionHook {
    pub fn new() -> Self {
        Self { window: ContextWindow::new() }
    }
}

impl AgentHook for ContextCompactionHook {
    fn name(&self) -> Option<String> {
        Some(String::from("ContextCompactionHook"))
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
}
