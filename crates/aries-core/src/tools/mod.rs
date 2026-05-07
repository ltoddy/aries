pub mod agent;
pub mod apply_patch;
pub mod bash;
pub mod batch;
pub mod codesearch;
pub mod edit;
pub mod glob;
pub mod grep;
pub mod ls;
pub mod lsp;
pub mod multiedit;
pub mod question;
pub mod read;
pub mod skill;
pub mod webfetch;
pub mod websearch;
pub mod write;

pub use agent::{AgentArgs, AgentOutput, AgentTool};
pub use apply_patch::{ApplyPatchArgs, ApplyPatchOutput, ApplyPatchTool};
pub use bash::{BashArgs, BashOutput, BashTool};
pub use batch::{BatchArgs, BatchOutput, BatchTool};
pub use codesearch::{CodeSearchArgs, CodeSearchOutput, CodeSearchTool};
pub use edit::{EditArgs, EditOutput, EditTool};
pub use glob::{GlobArgs, GlobOutput, GlobTool};
pub use grep::{GrepArgs, GrepOutput, GrepTool};
pub use ls::{LsArgs, LsOutput, LsTool};
pub use lsp::{LspArgs, LspOutput, LspTool};
pub use multiedit::{MultiEditArgs, MultiEditOutput, MultiEditTool};
pub use question::{AskUserQuestionArgs, AskUserQuestionOutput, AskUserQuestionTool};
pub use read::{ReadArgs, ReadOutput, ReadTool};
pub use skill::{SkillArgs, SkillOutput, SkillTool};
pub use webfetch::{WebFetchArgs, WebFetchOutput, WebFetchTool};
pub use websearch::{WebSearchArgs, WebSearchOutput, WebSearchTool};
pub use write::{WriteArgs, WriteOutput, WriteTool};

pub fn format_tool_output(tool_name: &str, raw_json: &str) -> String {
    let result = match tool_name {
        agent::NAME => serde_json::from_str::<AgentOutput>(raw_json).map(|o| o.to_string()),
        apply_patch::NAME => {
            serde_json::from_str::<ApplyPatchOutput>(raw_json).map(|o| o.to_string())
        },
        bash::NAME => serde_json::from_str::<BashOutput>(raw_json).map(|o| o.to_string()),
        batch::NAME => serde_json::from_str::<BatchOutput>(raw_json).map(|o| o.to_string()),
        codesearch::NAME => {
            serde_json::from_str::<CodeSearchOutput>(raw_json).map(|o| o.to_string())
        },
        edit::NAME => serde_json::from_str::<EditOutput>(raw_json).map(|o| o.to_string()),
        glob::NAME => serde_json::from_str::<GlobOutput>(raw_json).map(|o| o.to_string()),
        grep::NAME => serde_json::from_str::<GrepOutput>(raw_json).map(|o| o.to_string()),
        ls::NAME => serde_json::from_str::<LsOutput>(raw_json).map(|o| o.to_string()),
        lsp::NAME => serde_json::from_str::<LspOutput>(raw_json).map(|o| o.to_string()),
        multiedit::NAME => serde_json::from_str::<MultiEditOutput>(raw_json).map(|o| o.to_string()),
        question::NAME => {
            serde_json::from_str::<AskUserQuestionOutput>(raw_json).map(|o| o.to_string())
        },
        read::NAME => serde_json::from_str::<ReadOutput>(raw_json).map(|o| o.to_string()),
        skill::NAME => serde_json::from_str::<SkillOutput>(raw_json).map(|o| o.to_string()),
        webfetch::NAME => serde_json::from_str::<WebFetchOutput>(raw_json).map(|o| o.to_string()),
        websearch::NAME => serde_json::from_str::<WebSearchOutput>(raw_json).map(|o| o.to_string()),
        write::NAME => serde_json::from_str::<WriteOutput>(raw_json).map(|o| o.to_string()),
        _ => Ok(raw_json.to_string()),
    };

    result.unwrap_or_else(|_| raw_json.to_string())
}
