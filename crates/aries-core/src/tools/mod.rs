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
pub mod update_plan;
pub mod webfetch;
pub mod websearch;
pub mod write;

pub use crate::tools::agent::{AgentArgs, AgentOutput, AgentTool};
pub use crate::tools::bash::{BashArgs, BashOutput, BashTool};
pub use crate::tools::batch::{BatchArgs, BatchOutput, BatchTool};
pub use crate::tools::codesearch::{CodeSearchArgs, CodeSearchOutput, CodeSearchTool};
pub use crate::tools::edit::{EditArgs, EditOutput, EditTool};
pub use crate::tools::glob::{GlobArgs, GlobOutput, GlobTool};
pub use crate::tools::grep::{GrepArgs, GrepOutput, GrepTool};
pub use crate::tools::ls::{LsArgs, LsOutput, LsTool};
pub use crate::tools::lsp::{LspArgs, LspOutput, LspTool};
pub use crate::tools::multiedit::{MultiEditArgs, MultiEditOutput, MultiEditTool};
pub use crate::tools::question::{AskUserQuestionArgs, AskUserQuestionOutput, AskUserQuestionTool};
pub use crate::tools::read::{ReadArgs, ReadOutput, ReadTool};
pub use crate::tools::skill::{SkillArgs, SkillOutput, SkillTool};
pub use crate::tools::update_plan::{UpdatePlanArgs, UpdatePlanOutput, UpdatePlanTool};
pub use crate::tools::webfetch::{WebFetchArgs, WebFetchOutput, WebFetchTool};
pub use crate::tools::websearch::{WebSearchArgs, WebSearchOutput, WebSearchTool};
pub use crate::tools::write::{WriteArgs, WriteOutput, WriteTool};

pub const ALL_TOOL_NAMES: &[&str] = &[
    agent::NAME,
    bash::NAME,
    batch::NAME,
    codesearch::NAME,
    edit::NAME,
    glob::NAME,
    grep::NAME,
    ls::NAME,
    lsp::NAME,
    multiedit::NAME,
    question::NAME,
    read::NAME,
    skill::NAME,
    update_plan::NAME,
    webfetch::NAME,
    websearch::NAME,
    write::NAME,
];

pub fn create_tools(
    tool_names: &[&str],
    cwd: &std::path::Path,
    sender: &tokio::sync::mpsc::UnboundedSender<crate::event::AgentEvent>,
    lsp_client: Option<&aries_lspclient::SharedLspClient>,
    available_skills: &[aries_extension::skill::definition::SkillDefinition],
) -> Vec<Box<dyn rig_core::tool::ToolDyn>> {
    let cwd = cwd.to_path_buf();
    let mut tools: Vec<Box<dyn rig_core::tool::ToolDyn>> = Vec::with_capacity(tool_names.len());

    for &name in tool_names {
        let tool: Box<dyn rig_core::tool::ToolDyn> = match name {
            agent::NAME => continue, // generic over client, skip
            bash::NAME => Box::new(BashTool),
            batch::NAME => Box::new(BatchTool::new(cwd.clone())),
            codesearch::NAME => Box::new(CodeSearchTool),
            edit::NAME => Box::new(EditTool),
            glob::NAME => Box::new(GlobTool::new(cwd.clone())),
            grep::NAME => Box::new(GrepTool::new(cwd.clone())),
            ls::NAME => Box::new(LsTool::new(cwd.clone())),
            lsp::NAME => {
                if let Some(client) = lsp_client {
                    Box::new(LspTool::new(client.clone(), cwd.clone()))
                } else {
                    continue;
                }
            },
            multiedit::NAME => Box::new(MultiEditTool),
            question::NAME => Box::new(AskUserQuestionTool),
            read::NAME => Box::new(ReadTool),
            skill::NAME => {
                if available_skills.is_empty() {
                    continue;
                }
                Box::new(SkillTool::new(available_skills.to_vec()))
            },
            update_plan::NAME => Box::new(UpdatePlanTool::new(sender.clone())),
            webfetch::NAME => Box::new(WebFetchTool),
            websearch::NAME => Box::new(WebSearchTool),
            write::NAME => Box::new(WriteTool),
            _ => continue,
        };
        tools.push(tool);
    }

    tools
}

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
        update_plan::NAME => UpdatePlanArgs::render_args(raw_json),
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
        update_plan::NAME => UpdatePlanOutput::render_output(raw_json),
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
