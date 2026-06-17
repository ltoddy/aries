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

pub use self::common::{Effort, PostCompactTrigger};
pub use self::post_compact_hook_input::PostCompactHookInput;
pub use self::post_tool_use_failure_hook_input::PostToolUseFailureHookInput;
pub use self::post_tool_use_hook_input::PostToolUseHookInput;
pub use self::pre_compact_hook_input::PreCompactHookInput;
pub use self::pre_tool_use_hook_input::PreToolUseHookInput;
pub use self::session_end_hook_input::{SessionEndHookInput, SessionEndReason};
pub use self::session_start_hook_input::{SessionStartHookInput, SessionStartSource};
pub use self::stop_failure_hook_input::StopFailureHookInput;
pub use self::stop_hook_input::StopHookInput;
pub use self::subagent_start_hook_input::SubagentStartHookInput;
pub use self::subagent_stop_hook_input::SubagentStopHookInput;
pub use self::user_prompt_submit_hook_input::UserPromptSubmitHookInput;

pub trait HookInput: serde::Serialize {
    fn hook_event_name(&self) -> &'static str;
}
