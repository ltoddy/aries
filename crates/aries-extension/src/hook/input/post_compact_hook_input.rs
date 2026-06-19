use std::path::{Path, PathBuf};

use serde::{Serialize, Serializer};

use crate::hook::input::{HookInput, PostCompactTrigger};

const HOOK_EVENT_NAME: &str = "PostCompact";

#[derive(Debug, Default, Clone, Serialize)]
pub struct PostCompactHookInput {
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transcript_path: Option<PathBuf>,
    pub cwd: PathBuf,
    #[serde(serialize_with = "serialize_hook_event_name")]
    hook_event_name: String,
    pub trigger: PostCompactTrigger,
    pub compact_summary: String,
}

impl PostCompactHookInput {
    pub fn new(
        session_id: impl Into<String>,
        cwd: impl AsRef<Path>,
        trigger: PostCompactTrigger,
        compact_summary: impl Into<String>,
    ) -> Self {
        let session_id = session_id.into();
        let cwd = cwd.as_ref().to_path_buf();
        let compact_summary = compact_summary.into();

        Self { session_id, cwd, trigger, compact_summary, ..Default::default() }
    }

    pub fn transcript_path(mut self, transcript_path: impl AsRef<Path>) -> Self {
        self.transcript_path = Some(transcript_path.as_ref().to_path_buf());
        self
    }
}

fn serialize_hook_event_name<S: Serializer>(_: &String, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(HOOK_EVENT_NAME)
}

impl HookInput for PostCompactHookInput {
    fn hook_event_name(&self) -> &'static str {
        HOOK_EVENT_NAME
    }
}
