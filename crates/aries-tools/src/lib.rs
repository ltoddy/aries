pub mod context;
pub mod tools;

use std::path::Path;

use aries_event::Notifier;
use aries_extension::skill::SkillDefinition;
use aries_mode::Mode;
use itertools::Itertools;
use rig::tool::ToolSet;
pub use tools::*;

pub const ALL_TOOL_NAMES: &[&str] = &[
    agent::NAME,
    bash::NAME,
    batch::NAME,
    codesearch::NAME,
    edit::NAME,
    glob::NAME,
    grep::NAME,
    lsp::NAME,
    monitor::NAME,
    multiedit::NAME,
    question::NAME,
    read::NAME,
    skill::NAME,
    task_output::NAME,
    task_stop::NAME,
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
    notifier: Notifier,
) -> ToolSet {
    let tool_names = tool_names_from_mode(mode);

    create_tools_from_tool_names(&tool_names, cwd, lsp_client, skills, notifier)
}

pub fn tool_names_from_mode(mode: Mode) -> Vec<&'static str> {
    let mut tool_names = vec![
        bash::NAME,
        read::NAME,
        glob::NAME,
        grep::NAME,
        codesearch::NAME,
        webfetch::NAME,
        websearch::NAME,
        task_output::NAME,
        task_stop::NAME,
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
            monitor::NAME,
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
    notifier: Notifier,
) -> ToolSet {
    let cwd = cwd.as_ref();
    let tool_names = tool_names.iter().unique().collect_vec();
    let mut tool_set = ToolSet::default();

    let ctx = context::ToolContext::new(lsp_client.clone(), notifier);

    for &tool_name in tool_names {
        match tool_name {
            bash::NAME => {
                tool_set.add_tool(bash::BashTool::new(cwd, ctx.clone()));
            },
            batch::NAME => {
                tool_set.add_tool(batch::BatchTool::new(cwd, ctx.clone()));
            },
            codesearch::NAME => {
                tool_set.add_tool(codesearch::CodeSearchTool::new());
            },
            edit::NAME => {
                tool_set.add_tool(edit::EditTool::new(cwd, ctx.clone()));
            },
            glob::NAME => {
                tool_set.add_tool(glob::GlobTool::new(cwd));
            },
            grep::NAME => {
                tool_set.add_tool(grep::GrepTool::new(cwd));
            },
            lsp::NAME => {
                if let Some(lsp_client) = lsp_client.clone() {
                    tool_set.add_tool(lsp::LspTool::new(lsp_client, cwd));
                }
            },
            monitor::NAME => {
                tool_set.add_tool(monitor::MonitorTool::new(cwd, ctx.clone()));
            },
            multiedit::NAME => {
                tool_set.add_tool(multiedit::MultiEditTool::new(cwd, ctx.clone()));
            },
            question::NAME => {
                tool_set.add_tool(question::AskUserQuestionTool::new());
            },
            read::NAME => {
                tool_set.add_tool(read::ReadTool::new(cwd, ctx.clone()));
            },
            skill::NAME => {
                if skills.is_empty() {
                    continue;
                }
                tool_set.add_tool(skill::SkillTool::new(skills.to_vec()));
            },
            task_output::NAME => {
                tool_set.add_tool(task_output::TaskOutputTool::new(ctx.clone()));
            },
            task_stop::NAME => {
                tool_set.add_tool(task_stop::TaskStopTool::new(ctx.clone()));
            },
            update_plan::NAME => {
                tool_set.add_tool(update_plan::UpdatePlanTool::new());
            },
            webfetch::NAME => {
                tool_set.add_tool(webfetch::WebFetchTool::new());
            },
            websearch::NAME => {
                tool_set.add_tool(websearch::WebSearchTool::new());
            },
            write::NAME => {
                tool_set.add_tool(write::WriteTool::new(cwd, ctx.clone()));
            },
            _ => {},
        };
    }

    tool_set
}
