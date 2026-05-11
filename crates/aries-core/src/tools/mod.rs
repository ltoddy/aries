pub mod agent;
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

pub use self::agent::{AgentArgs, AgentOutput, AgentTool};
pub use self::bash::{BashArgs, BashOutput, BashTool};
pub use self::batch::{BatchArgs, BatchOutput, BatchTool};
pub use self::codesearch::{CodeSearchArgs, CodeSearchOutput, CodeSearchTool};
pub use self::edit::{EditArgs, EditOutput, EditTool};
pub use self::glob::{GlobArgs, GlobOutput, GlobTool};
pub use self::grep::{GrepArgs, GrepOutput, GrepTool};
pub use self::ls::{LsArgs, LsOutput, LsTool};
pub use self::lsp::{LspArgs, LspOutput, LspTool};
pub use self::multiedit::{MultiEditArgs, MultiEditOutput, MultiEditTool};
pub use self::question::{AskUserQuestionArgs, AskUserQuestionOutput, AskUserQuestionTool};
pub use self::read::{ReadArgs, ReadOutput, ReadTool};
pub use self::skill::{SkillArgs, SkillOutput, SkillTool};
pub use self::webfetch::{WebFetchArgs, WebFetchOutput, WebFetchTool};
pub use self::websearch::{WebSearchArgs, WebSearchOutput, WebSearchTool};
pub use self::write::{WriteArgs, WriteOutput, WriteTool};

pub fn format_tool_args(tool_name: &str, raw_json: &str) -> (String, Option<String>) {
    let result = match tool_name {
        agent::NAME => AgentArgs::render_args(raw_json),
        bash::NAME => BashArgs::render_args(raw_json),
        batch::NAME => BatchArgs::render_args(raw_json),
        codesearch::NAME => CodeSearchArgs::render_args(raw_json),
        edit::NAME => EditArgs::render_args(raw_json),
        glob::NAME => GlobArgs::render_args(raw_json),
        grep::NAME => GrepArgs::render_args(raw_json),
        ls::NAME => LsArgs::render_args(raw_json),
        lsp::NAME => LspArgs::render_args(raw_json),
        multiedit::NAME => MultiEditArgs::render_args(raw_json),
        question::NAME => AskUserQuestionArgs::render_args(raw_json),
        read::NAME => ReadArgs::render_args(raw_json),
        skill::NAME => SkillArgs::render_args(raw_json),
        webfetch::NAME => WebFetchArgs::render_args(raw_json),
        websearch::NAME => WebSearchArgs::render_args(raw_json),
        write::NAME => WriteArgs::render_args(raw_json),
        _ => Ok((raw_json.to_string(), None)),
    };

    result.unwrap_or_else(|_| (raw_json.to_string(), None))
}

pub fn format_tool_output(tool_name: &str, raw_json: &str) -> String {
    let result = match tool_name {
        agent::NAME => AgentOutput::render_output(raw_json),
        bash::NAME => BashOutput::render_output(raw_json),
        batch::NAME => BatchOutput::render_output(raw_json),
        codesearch::NAME => CodeSearchOutput::render_output(raw_json),
        edit::NAME => EditOutput::render_output(raw_json),
        glob::NAME => GlobOutput::render_output(raw_json),
        grep::NAME => GrepOutput::render_output(raw_json),
        ls::NAME => LsOutput::render_output(raw_json),
        lsp::NAME => LspOutput::render_output(raw_json),
        multiedit::NAME => MultiEditOutput::render_output(raw_json),
        question::NAME => AskUserQuestionOutput::render_output(raw_json),
        read::NAME => ReadOutput::render_output(raw_json),
        skill::NAME => SkillOutput::render_output(raw_json),
        webfetch::NAME => WebFetchOutput::render_output(raw_json),
        websearch::NAME => WebSearchOutput::render_output(raw_json),
        write::NAME => WriteOutput::render_output(raw_json),
        _ => Ok(raw_json.to_string()),
    };

    result.unwrap_or_else(|_| raw_json.to_string())
}

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("failed to deserialize tool data: {0}")]
    Deserialize(#[from] serde_json::Error),
}

pub trait ToolArgsRender {
    fn render_args(raw: &str) -> Result<(String, Option<String>), RenderError>;
}

pub trait ToolOutputRender {
    fn render_output(raw: &str) -> Result<String, RenderError>;
}
