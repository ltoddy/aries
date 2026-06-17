use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};

use serde::{Serialize, Serializer};

use crate::ext::hook::input::HookInput;

const HOOK_EVENT_NAME: &str = "SessionEnd";

#[derive(Debug, Default, Clone, Serialize)]
pub struct SessionEndHookInput {
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transcript_path: Option<PathBuf>,
    pub cwd: PathBuf,
    #[serde(serialize_with = "serialize_hook_event_name")]
    hook_event_name: String,
    pub reason: SessionEndReason,
}

impl SessionEndHookInput {
    pub fn new(
        session_id: impl Into<String>,
        cwd: impl AsRef<Path>,
        reason: SessionEndReason,
    ) -> Self {
        let session_id = session_id.into();
        let cwd = cwd.as_ref().to_path_buf();
        let reason = reason.into();

        Self { session_id, cwd, reason, ..Default::default() }
    }

    pub fn transcript_path(mut self, transcript_path: impl AsRef<Path>) -> Self {
        self.transcript_path = Some(transcript_path.as_ref().to_path_buf());
        self
    }
}

impl HookInput for SessionEndHookInput {
    fn hook_event_name(&self) -> &'static str {
        HOOK_EVENT_NAME
    }
}

fn serialize_hook_event_name<S: Serializer>(_: &String, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(HOOK_EVENT_NAME)
}

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionEndReason {
    Clear,
    Resume,
    #[default]
    Logout,
    PromptInputExit,
    BypassPermissionsDisabled,
    Other,
}

impl Display for SessionEndReason {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionEndReason::Clear => write!(f, "clear"),
            SessionEndReason::Resume => write!(f, "resume"),
            SessionEndReason::Logout => write!(f, "logout"),
            SessionEndReason::PromptInputExit => write!(f, "prompt_input_exit"),
            SessionEndReason::BypassPermissionsDisabled => write!(f, "bypass_permissions_disabled"),
            SessionEndReason::Other => write!(f, "other"),
        }
    }
}
