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

use std::path::Path;

use aries_extension::skill::SkillDefinition;
use aries_mode::Mode;
use itertools::Itertools;
use rig_core::tool::ToolDyn;

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

pub fn create_tools_from_mode(
    mode: Mode,
    cwd: impl AsRef<Path>,
    lsp_client: Option<aries_lspclient::SharedLspClient>,
    skills: &[SkillDefinition],
) -> Vec<Box<dyn ToolDyn>> {
    let mut tool_names = vec![
        bash::NAME,
        read::NAME,
        glob::NAME,
        grep::NAME,
        ls::NAME,
        codesearch::NAME,
        webfetch::NAME,
        websearch::NAME,
    ];

    match mode {
        Mode::Build | Mode::General => tool_names.extend_from_slice(&[
            batch::NAME,
            edit::NAME,
            lsp::NAME,
            multiedit::NAME,
            question::NAME,
            skill::NAME,
            update_plan::NAME,
            write::NAME,
        ]),
        Mode::Plan => tool_names.push(question::NAME),
        Mode::Explore => {},
    }

    create_tools_from_tool_names(&tool_names, cwd, lsp_client, skills)
}

pub fn create_tools_from_tool_names(
    tool_names: &[&str],
    cwd: impl AsRef<Path>,
    lsp_client: Option<aries_lspclient::SharedLspClient>,
    skills: &[SkillDefinition],
) -> Vec<Box<dyn ToolDyn>> {
    let cwd = cwd.as_ref();
    let tool_names = tool_names.iter().unique().collect_vec();
    let mut tools = Vec::<Box<dyn ToolDyn>>::with_capacity(tool_names.len());

    for &tool_name in tool_names {
        match tool_name {
            bash::NAME => tools.push(Box::new(bash::BashTool::new())),
            batch::NAME => tools.push(Box::new(batch::BatchTool::new(cwd))),
            codesearch::NAME => tools.push(Box::new(codesearch::CodeSearchTool::new())),
            edit::NAME => tools.push(Box::new(edit::EditTool::new())),
            glob::NAME => tools.push(Box::new(glob::GlobTool::new(cwd))),
            ls::NAME => tools.push(Box::new(ls::LsTool::new(cwd))),
            lsp::NAME => {
                if let Some(lsp_client) = lsp_client.clone() {
                    tools.push(Box::new(lsp::LspTool::new(lsp_client, cwd)))
                }
            },
            multiedit::NAME => tools.push(Box::new(multiedit::MultiEditTool::new())),
            question::NAME => tools.push(Box::new(question::AskUserQuestionTool::new())),
            read::NAME => tools.push(Box::new(read::ReadTool::new())),
            skill::NAME => {
                if skills.is_empty() {
                    continue;
                }
                tools.push(Box::new(skill::SkillTool::new(skills.to_vec())));
            },
            update_plan::NAME => tools.push(Box::new(update_plan::UpdatePlanTool::new())),
            webfetch::NAME => tools.push(Box::new(webfetch::WebFetchTool::new())),
            websearch::NAME => tools.push(Box::new(websearch::WebSearchTool::new())),
            write::NAME => tools.push(Box::new(write::WriteTool::new())),
            _ => {},
        }
    }

    tools
}
