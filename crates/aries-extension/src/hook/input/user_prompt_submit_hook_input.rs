use std::path::{Path, PathBuf};

use serde::{Serialize, Serializer};

use crate::hook::input::HookInput;

const HOOK_EVENT_NAME: &str = "UserPromptSubmit";

#[derive(Debug, Default, Clone, Serialize)]
pub struct UserPromptSubmitHookInput {
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transcript_path: Option<PathBuf>,
    pub cwd: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission_mode: Option<String>,
    #[serde(serialize_with = "serialize_hook_event_name")]
    hook_event_name: String,
    pub prompt: String,
}

impl UserPromptSubmitHookInput {
    pub fn new(
        session_id: impl Into<String>,
        cwd: impl AsRef<Path>,
        prompt: impl Into<String>,
    ) -> Self {
        let session_id = session_id.into();
        let cwd = cwd.as_ref().to_owned();
        let prompt = prompt.into();

        Self { session_id, cwd, prompt, ..Default::default() }
    }

    pub fn transcript_path(mut self, transcript_path: impl AsRef<Path>) -> Self {
        self.transcript_path = Some(transcript_path.as_ref().to_owned());
        self
    }

    pub fn permission_mode(mut self, permission_mode: impl Into<String>) -> Self {
        self.permission_mode = Some(permission_mode.into());
        self
    }
}

impl HookInput for UserPromptSubmitHookInput {
    fn hook_event_name(&self) -> &'static str {
        HOOK_EVENT_NAME
    }
}

fn serialize_hook_event_name<S: Serializer>(_: &String, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(HOOK_EVENT_NAME)
}
