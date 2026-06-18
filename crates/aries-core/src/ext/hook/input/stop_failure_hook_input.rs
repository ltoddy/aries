use std::path::{Path, PathBuf};

use serde::{Serialize, Serializer};

use crate::ext::hook::input::HookInput;

const HOOK_EVENT_NAME: &str = "StopFailure";

#[derive(Debug, Default, Clone, Serialize)]
pub struct StopFailureHookInput {
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transcript_path: Option<PathBuf>,
    pub cwd: PathBuf,
    #[serde(serialize_with = "serialize_hook_event_name")]
    hook_event_name: String,
    /// Error type, for example `rate_limit` or `server_error`. Used for matcher
    /// filtering.
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_details: Option<String>,
    /// The rendered error text shown in the conversation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_assistant_message: Option<String>,
}

impl StopFailureHookInput {
    pub fn new(
        session_id: impl Into<String>,
        cwd: impl AsRef<Path>,
        error: impl Into<String>,
    ) -> Self {
        let session_id = session_id.into();
        let cwd = cwd.as_ref().to_path_buf();
        let error = error.into();

        Self { session_id, cwd, error, ..Default::default() }
    }

    pub fn transcript_path(mut self, transcript_path: impl AsRef<Path>) -> Self {
        self.transcript_path = Some(transcript_path.as_ref().to_path_buf());
        self
    }

    pub fn error_details(mut self, error_details: impl Into<String>) -> Self {
        self.error_details = Some(error_details.into());
        self
    }

    pub fn last_assistant_message(mut self, last_assistant_message: impl Into<String>) -> Self {
        self.last_assistant_message = Some(last_assistant_message.into());
        self
    }
}

impl HookInput for StopFailureHookInput {
    fn hook_event_name(&self) -> &'static str {
        HOOK_EVENT_NAME
    }
}

fn serialize_hook_event_name<S: Serializer>(_: &String, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(HOOK_EVENT_NAME)
}
