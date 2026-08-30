use std::path::{Path, PathBuf};

use serde::{Serialize, Serializer};

use crate::hook::input::HookInput;

const HOOK_EVENT_NAME: &str = "SubagentStart";

#[derive(Debug, Default, Clone, Serialize)]
pub struct SubagentStartHookInput {
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transcript_path: Option<PathBuf>,
    pub cwd: PathBuf,
    #[serde(serialize_with = "serialize_hook_event_name")]
    hook_event_name: String,
    pub agent_id: String,
    pub agent_type: String,
}

impl SubagentStartHookInput {
    pub fn new(
        session_id: impl Into<String>,
        cwd: impl AsRef<Path>,
        agent_id: impl Into<String>,
        agent_type: impl Into<String>,
    ) -> Self {
        let session_id = session_id.into();
        let cwd = cwd.as_ref().to_owned();
        let agent_id = agent_id.into();
        let agent_type = agent_type.into();

        Self { session_id, cwd, agent_id, agent_type, ..Default::default() }
    }

    pub fn transcript_path(mut self, transcript_path: impl AsRef<Path>) -> Self {
        self.transcript_path = Some(transcript_path.as_ref().to_owned());
        self
    }
}

impl HookInput for SubagentStartHookInput {
    fn hook_event_name(&self) -> &'static str {
        HOOK_EVENT_NAME
    }
}

fn serialize_hook_event_name<S: Serializer>(_: &String, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(HOOK_EVENT_NAME)
}
