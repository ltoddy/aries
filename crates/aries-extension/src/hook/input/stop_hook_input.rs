use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::hook::input::HookInput;
use crate::hook::input::common::Effort;

const HOOK_EVENT_NAME: &str = "Stop";

#[derive(Debug, Default, Clone, Serialize)]
pub struct StopHookInput {
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_assistant_message: Option<String>,
}

impl StopHookInput {
    pub fn new(
        session_id: impl Into<String>,
        cwd: impl AsRef<Path>,
        stop_hook_active: bool,
    ) -> Self {
        let session_id = session_id.into();
        let cwd = cwd.as_ref().to_owned();

        Self { session_id, cwd, stop_hook_active, ..Default::default() }
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

    pub fn last_assistant_message(mut self, last_assistant_message: impl Into<String>) -> Self {
        self.last_assistant_message = Some(last_assistant_message.into());
        self
    }
}

impl HookInput for StopHookInput {
    fn hook_event_name(&self) -> &'static str {
        HOOK_EVENT_NAME
    }
}

fn serialize_hook_event_name<S: serde::Serializer>(_: &String, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(HOOK_EVENT_NAME)
}
