use std::fmt::Debug;
use std::path::{Path, PathBuf};

use serde::{Serialize, Serializer};

use crate::ext::hook::input::HookInput;
use crate::ext::hook::input::common::Effort;

const HOOK_EVENT_NAME: &str = "PostToolUseFailure";

#[derive(Debug, Default, Clone, Serialize)]
pub struct PostToolUseFailureHookInput<ToolInput>
where
    ToolInput: Serialize + Clone + Debug,
{
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transcript_path: Option<PathBuf>,
    pub cwd: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<Effort>,
    #[serde(serialize_with = "serialize_hook_event_name")]
    hook_event_name: String,
    pub tool_name: String,
    pub tool_input: ToolInput,
    pub tool_use_id: String,
    pub error: String,
    /// Whether the failure was caused by user interruption.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_interrupt: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

impl<ToolInput> PostToolUseFailureHookInput<ToolInput>
where
    ToolInput: Serialize + Clone + Debug + Default,
{
    pub fn new(
        session_id: impl Into<String>,
        cwd: impl AsRef<Path>,
        tool_name: impl Into<String>,
        tool_input: ToolInput,
        tool_use_id: impl Into<String>,
        error: impl Into<String>,
    ) -> Self {
        let session_id = session_id.into();
        let cwd = cwd.as_ref().to_path_buf();
        let tool_name = tool_name.into();
        let tool_use_id = tool_use_id.into();
        let error = error.into();

        Self { session_id, cwd, tool_name, tool_input, tool_use_id, error, ..Default::default() }
    }

    pub fn transcript_path(mut self, transcript_path: impl AsRef<Path>) -> Self {
        self.transcript_path = Some(transcript_path.as_ref().to_path_buf());
        self
    }

    pub fn permission_mode(mut self, permission_mode: impl Into<String>) -> Self {
        self.permission_mode = Some(permission_mode.into());
        self
    }

    pub fn effort(mut self, effort: Effort) -> Self {
        self.effort = Some(effort);
        self
    }

    pub fn is_interrupt(mut self, is_interrupt: bool) -> Self {
        self.is_interrupt = Some(is_interrupt);
        self
    }

    pub fn duration_ms(mut self, duration_ms: u64) -> Self {
        self.duration_ms = Some(duration_ms);
        self
    }
}

impl<ToolInput> HookInput for PostToolUseFailureHookInput<ToolInput>
where
    ToolInput: Serialize + Clone + Debug,
{
    fn hook_event_name(&self) -> &'static str {
        HOOK_EVENT_NAME
    }
}

fn serialize_hook_event_name<S: Serializer>(_: &String, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(HOOK_EVENT_NAME)
}
