use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};

use serde::{Serialize, Serializer};

use crate::hook::input::{HookInput, PostCompactTrigger};

const HOOK_EVENT_NAME: &str = "PreCompact";

#[derive(Debug, Default, Clone, Serialize)]
pub struct PreCompactHookInput {
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transcript_path: Option<PathBuf>,
    pub cwd: PathBuf,
    #[serde(serialize_with = "serialize_hook_event_name")]
    hook_event_name: String,
    pub trigger: PostCompactTrigger,
    pub custom_instructions: PreCompactCustomInstructions,
}

impl PreCompactHookInput {
    pub fn new(
        session_id: impl Into<String>,
        cwd: impl AsRef<Path>,
        trigger: PostCompactTrigger,
        custom_instructions: PreCompactCustomInstructions,
    ) -> Self {
        let session_id = session_id.into();
        let cwd = cwd.as_ref().to_path_buf();

        Self { session_id, cwd, trigger, custom_instructions, ..Default::default() }
    }

    pub fn transcript_path(mut self, transcript_path: impl AsRef<Path>) -> Self {
        self.transcript_path = Some(transcript_path.as_ref().to_path_buf());
        self
    }
}

impl HookInput for PreCompactHookInput {
    fn hook_event_name(&self) -> &'static str {
        HOOK_EVENT_NAME
    }
}

fn serialize_hook_event_name<S: Serializer>(_: &String, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(HOOK_EVENT_NAME)
}

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename = "snake_case")]
pub enum PreCompactCustomInstructions {
    #[default]
    Auto,
    Manual,
}

impl Display for PreCompactCustomInstructions {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            PreCompactCustomInstructions::Auto => write!(f, "auto"),
            PreCompactCustomInstructions::Manual => write!(f, "manual"),
        }
    }
}
