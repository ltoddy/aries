pub mod context;
pub mod tools;

use std::path::Path;

use aries_extension::skill::SkillDefinition;
use aries_mode::Mode;
use itertools::Itertools;
use rig_core::tool::ToolDyn;
pub use tools::*;

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

pub fn is_builtin_tool(tool_name: &str) -> bool {
    ALL_TOOL_NAMES.contains(&tool_name)
}

pub fn create_tools_from_mode(
    mode: Mode,
    cwd: impl AsRef<Path>,
    lsp_client: Option<aries_lspclient::SharedLspClient>,
    skills: &[SkillDefinition],
) -> Vec<Box<dyn ToolDyn>> {
    let tool_names = tool_names_from_mode(mode);

    create_tools_from_tool_names(&tool_names, cwd, lsp_client, skills)
}

pub fn tool_names_from_mode(mode: Mode) -> Vec<&'static str> {
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

    tool_names
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

    let ctx = context::ToolContext::new(lsp_client.clone());

    for &tool_name in tool_names {
        match tool_name {
            bash::NAME => tools.push(Box::new(bash::BashTool::new())),
            batch::NAME => tools.push(Box::new(batch::BatchTool::new(cwd, ctx.clone()))),
            codesearch::NAME => tools.push(Box::new(codesearch::CodeSearchTool::new())),
            edit::NAME => tools.push(Box::new(edit::EditTool::new(cwd, ctx.clone()))),
            glob::NAME => tools.push(Box::new(glob::GlobTool::new(cwd))),
            grep::NAME => tools.push(Box::new(grep::GrepTool::new(cwd.to_path_buf()))),
            ls::NAME => tools.push(Box::new(ls::LsTool::new(cwd))),
            lsp::NAME => {
                if let Some(lsp_client) = lsp_client.clone() {
                    tools.push(Box::new(lsp::LspTool::new(lsp_client, cwd)))
                }
            },
            multiedit::NAME => {
                tools.push(Box::new(multiedit::MultiEditTool::new(cwd, ctx.clone())))
            },
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
            write::NAME => tools.push(Box::new(write::WriteTool::new(cwd, ctx.clone()))),
            _ => {},
        }
    }

    tools
}
