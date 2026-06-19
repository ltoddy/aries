use std::path::{Path, PathBuf};

use serde::{Serialize, Serializer};

use crate::hook::input::HookInput;
use crate::hook::input::common::Effort;

const HOOK_EVENT_NAME: &str = "SubagentStop";

#[derive(Debug, Default, Clone, Serialize)]
pub struct SubagentStopHookInput {
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
    /// `true` when already continuing as a result of a stop hook.
    pub stop_hook_active: bool,
    pub agent_id: String,
    pub agent_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_transcript_path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_assistant_message: Option<String>,
}

impl SubagentStopHookInput {
    pub fn new(
        session_id: impl Into<String>,
        cwd: impl AsRef<Path>,
        stop_hook_active: bool,
        agent_id: impl Into<String>,
        agent_type: impl Into<String>,
    ) -> Self {
        let session_id = session_id.into();
        let cwd = cwd.as_ref().to_path_buf();
        let agent_id = agent_id.into();
        let agent_type = agent_type.into();

        Self { session_id, cwd, stop_hook_active, agent_id, agent_type, ..Default::default() }
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

    pub fn agent_transcript_path(mut self, agent_transcript_path: impl AsRef<Path>) -> Self {
        self.agent_transcript_path = Some(agent_transcript_path.as_ref().to_path_buf());
        self
    }

    pub fn last_assistant_message(mut self, last_assistant_message: impl Into<String>) -> Self {
        self.last_assistant_message = Some(last_assistant_message.into());
        self
    }
}

impl HookInput for SubagentStopHookInput {
    fn hook_event_name(&self) -> &'static str {
        HOOK_EVENT_NAME
    }
}

fn serialize_hook_event_name<S: Serializer>(_: &String, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(HOOK_EVENT_NAME)
}
