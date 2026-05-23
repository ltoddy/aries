use std::fmt::Debug;
use std::path::PathBuf;

use serde::{Serialize, Serializer};

#[derive(Debug, Clone, Serialize)]
pub struct PostToolUseHookInput<ToolInput, ToolResponse>
where
    ToolInput: Serialize + Clone + Debug,
    ToolResponse: Serialize + Clone + Debug,
{
    pub session_id: String,
    pub transcript_path: PathBuf,
    pub cwd: PathBuf,
    pub permission_mode: Option<String>,
    pub agent_id: Option<String>,
    pub agent_type: Option<String>,
    #[serde(serialize_with = "serialize_hook_event_name")]
    pub hook_event_name: String,
    pub tool_name: String,
    pub tool_input: ToolInput,
    pub tool_response: ToolResponse,
    pub tool_use_id: String,
}

fn serialize_hook_event_name<S: Serializer>(_: &String, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str("PostToolUse")
}
