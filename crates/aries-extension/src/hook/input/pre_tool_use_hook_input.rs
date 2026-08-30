use std::fmt::Debug;
use std::path::{Path, PathBuf};

use serde::{Serialize, Serializer};

use crate::hook::input::{Effort, HookInput};

const HOOK_EVENT_NAME: &str = "PreToolUse";

#[derive(Debug, Default, Clone, Serialize)]
pub struct PreToolUseHookInput<ToolInput>
where
    ToolInput: Clone + Debug + Default + Serialize,
{
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transcript_path: Option<PathBuf>,
    pub cwd: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<Effort>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_type: Option<String>,
    #[serde(serialize_with = "serialize_hook_event_name")]
    hook_event_name: String,
    pub tool_name: String,
    pub tool_input: ToolInput,
    pub tool_use_id: String,
}

impl<ToolInput> PreToolUseHookInput<ToolInput>
where
    ToolInput: Clone + Debug + Default + Serialize,
{
    pub fn new(
        session_id: impl Into<String>,
        cwd: impl AsRef<Path>,
        tool_name: impl Into<String>,
        tool_input: ToolInput,
        tool_use_id: impl Into<String>,
    ) -> Self {
        let session_id = session_id.into();
        let cwd = cwd.as_ref().to_owned();
        let tool_name = tool_name.into();
        let tool_use_id = tool_use_id.into();

        Self { session_id, cwd, tool_name, tool_input, tool_use_id, ..Default::default() }
    }

    pub fn transcript_path(mut self, transcript_path: impl AsRef<Path>) -> Self {
        self.transcript_path = Some(transcript_path.as_ref().to_owned());
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

    pub fn agent_id(mut self, agent_id: impl Into<String>) -> Self {
        self.agent_id = Some(agent_id.into());
        self
    }

    pub fn agent_type(mut self, agent_type: impl Into<String>) -> Self {
        self.agent_type = Some(agent_type.into());
        self
    }
}

impl<ToolInput> HookInput for PreToolUseHookInput<ToolInput>
where
    ToolInput: Clone + Debug + Default + Serialize,
{
    fn hook_event_name(&self) -> &'static str {
        HOOK_EVENT_NAME
    }
}

fn serialize_hook_event_name<S: Serializer>(_: &String, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(HOOK_EVENT_NAME)
}
