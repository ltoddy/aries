mod common;
mod post_compact_hook_input;
mod post_tool_use_failure_hook_input;
mod post_tool_use_hook_input;
mod pre_compact_hook_input;
mod pre_tool_use_hook_input;
mod session_end_hook_input;
mod session_start_hook_input;
mod stop_failure_hook_input;
mod stop_hook_input;
mod subagent_start_hook_input;
mod subagent_stop_hook_input;
mod user_prompt_submit_hook_input;

pub use crate::ext::hook::input::common::{Effort, PostCompactTrigger};
pub use crate::ext::hook::input::post_compact_hook_input::PostCompactHookInput;
pub use crate::ext::hook::input::post_tool_use_failure_hook_input::PostToolUseFailureHookInput;
pub use crate::ext::hook::input::post_tool_use_hook_input::PostToolUseHookInput;
pub use crate::ext::hook::input::pre_compact_hook_input::PreCompactHookInput;
pub use crate::ext::hook::input::pre_tool_use_hook_input::PreToolUseHookInput;
pub use crate::ext::hook::input::session_end_hook_input::{SessionEndHookInput, SessionEndReason};
pub use crate::ext::hook::input::session_start_hook_input::{
    SessionStartHookInput, SessionStartSource,
};
pub use crate::ext::hook::input::stop_failure_hook_input::StopFailureHookInput;
pub use crate::ext::hook::input::stop_hook_input::StopHookInput;
pub use crate::ext::hook::input::subagent_start_hook_input::SubagentStartHookInput;
pub use crate::ext::hook::input::subagent_stop_hook_input::SubagentStopHookInput;
pub use crate::ext::hook::input::user_prompt_submit_hook_input::UserPromptSubmitHookInput;

pub trait HookInput: serde::Serialize {
    fn hook_event_name(&self) -> &'static str;
}
