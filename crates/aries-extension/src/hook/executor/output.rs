use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookOutput {
    #[serde(rename = "continue", default = "default_true")]
    pub should_continue: bool,

    // should_continue 为 false 时进行展示 (不添加到上下文中)
    #[serde(rename = "stopReason", default, skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,

    #[serde(rename = "suppressOutput", default)]
    pub suppress_output: bool,

    #[serde(rename = "systemMessage", default, skip_serializing_if = "Option::is_none")]
    pub system_message: Option<String>,

    #[serde(rename = "terminalSequence", default, skip_serializing_if = "Option::is_none")]
    pub terminal_sequence: Option<String>,

    // 唯一值为 "block"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision: Option<String>,

    // decision 为 block 时,添加到上下文中
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,

    #[serde(rename = "hookSpecificOutput", default, skip_serializing_if = "Option::is_none")]
    pub hook_specific_output: Option<HookSpecificOutput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookSpecificOutput {
    #[serde(rename = "hookEventName", default, skip_serializing_if = "Option::is_none")]
    pub hook_event_name: Option<String>,

    #[serde(rename = "additionalContext", default, skip_serializing_if = "Option::is_none")]
    pub additional_context: Option<String>,
}

impl HookOutput {
    pub fn additional_context(&self, hook_event_name: impl AsRef<str>) -> Option<String> {
        let hook_event_name = hook_event_name.as_ref();

        let hook_specific = self.hook_specific_output.as_ref()?;
        if hook_specific.hook_event_name.as_deref().is_some_and(|name| name != hook_event_name) {
            return None;
        }

        hook_specific.additional_context.clone()
    }
}

fn default_true() -> bool {
    true
}
