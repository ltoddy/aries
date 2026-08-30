use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};

use serde::{Serialize, Serializer};

use crate::hook::input::HookInput;

const HOOK_EVENT_NAME: &str = "SessionStart";

#[derive(Debug, Default, Clone, Serialize)]
pub struct SessionStartHookInput {
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transcript_path: Option<PathBuf>,
    pub cwd: PathBuf,
    #[serde(serialize_with = "serialize_hook_event_name")]
    hook_event_name: String,
    pub source: SessionStartSource,
    pub model: String,
    pub agent_type: String,
    /// Current session title, if one is already set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_title: Option<String>,
}

impl SessionStartHookInput {
    pub fn new(
        session_id: impl Into<String>,
        cwd: impl AsRef<Path>,
        source: SessionStartSource,
        model: impl Into<String>,
        agent_type: impl Into<String>,
    ) -> Self {
        let session_id = session_id.into();
        let cwd = cwd.as_ref().to_owned();
        let model = model.into();
        let agent_type = agent_type.into();

        Self { session_id, cwd, source, model, agent_type, ..Default::default() }
    }

    pub fn transcript_path(mut self, transcript_path: impl AsRef<Path>) -> Self {
        self.transcript_path = Some(transcript_path.as_ref().to_owned());
        self
    }

    pub fn session_title(mut self, session_title: impl Into<String>) -> Self {
        self.session_title = Some(session_title.into());
        self
    }
}

impl HookInput for SessionStartHookInput {
    fn hook_event_name(&self) -> &'static str {
        HOOK_EVENT_NAME
    }
}

fn serialize_hook_event_name<S: Serializer>(_: &String, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(HOOK_EVENT_NAME)
}

#[derive(Debug, Default, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStartSource {
    #[default]
    Startup,
    Resume,
    Clear,
    Compact,
}

impl Display for SessionStartSource {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionStartSource::Startup => write!(f, "startup"),
            SessionStartSource::Resume => write!(f, "resume"),
            SessionStartSource::Clear => write!(f, "clear"),
            SessionStartSource::Compact => write!(f, "compact"),
        }
    }
}
