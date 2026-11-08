pub mod agent;
pub mod bash;
pub mod batch;
pub mod codesearch;
pub mod context;
mod diff;
pub mod edit;
pub mod enter_worktree;
pub mod exit_worktree;
pub mod glob;
pub mod grep;
pub mod lsp;
pub mod monitor;
pub mod multiedit;
pub mod queue;
pub mod read;
pub mod read_state;
pub mod skill;
pub mod task;
pub mod task_output;
pub mod task_stop;
pub mod update_plan;
pub mod webfetch;
pub mod websearch;
pub mod worktree;
pub mod write;

use std::path::Path;

use aries_communicate::Notifier;
use aries_extension::AgentExtensions;
use aries_init::GlobalContext;
use git2::Repository;
use itertools::Itertools;
use rig::Model;
use rig::providers::openai::wire::OpenAiWire;
use rig::tool::ToolSet;
use toasty::Db;

use self::agent as agent_tool;
use self::agent::AgentOutput;
use self::bash::BashOutput;
use self::batch::BatchOutput;
use self::codesearch::CodeSearchOutput;
pub use self::context::ToolContext;
use self::edit::EditOutput;
use self::enter_worktree::EnterGitWorktreeOutput;
use self::exit_worktree::ExitGitWorktreeOutput;
use self::glob::GlobOutput;
use self::grep::GrepOutput;
use self::lsp::LspOutput;
use self::monitor::MonitorOutput;
use self::multiedit::MultiEditOutput;
use self::read::ReadOutput;
use self::skill::SkillOutput;
use self::task_output::TaskOutputOutput;
use self::task_stop::TaskStopOutput;
use self::update_plan::UpdatePlanOutput;
use self::webfetch::WebFetchOutput;
use self::websearch::WebSearchOutput;
use self::write::WriteOutput;

pub fn format_tool_output(tool_name: &str, raw_json: serde_json::Value) -> String {
    let result = match tool_name {
        agent::NAME => AgentOutput::render_output(raw_json),
        bash::NAME => BashOutput::render_output(raw_json),
        batch::NAME => BatchOutput::render_output(raw_json),
        codesearch::NAME => CodeSearchOutput::render_output(raw_json),
        edit::NAME => EditOutput::render_output(raw_json),
        enter_worktree::NAME => EnterGitWorktreeOutput::render_output(raw_json),
        exit_worktree::NAME => ExitGitWorktreeOutput::render_output(raw_json),
        glob::NAME => GlobOutput::render_output(raw_json),
        grep::NAME => GrepOutput::render_output(raw_json),
        lsp::NAME => LspOutput::render_output(raw_json),
        monitor::NAME => MonitorOutput::render_output(raw_json),
        multiedit::NAME => MultiEditOutput::render_output(raw_json),
        read::NAME => ReadOutput::render_output(raw_json),
        skill::NAME => SkillOutput::render_output(raw_json),
        task_output::NAME => TaskOutputOutput::render_output(raw_json),
        task_stop::NAME => TaskStopOutput::render_output(raw_json),
        update_plan::NAME => UpdatePlanOutput::render_output(raw_json),
        webfetch::NAME => WebFetchOutput::render_output(raw_json),
        websearch::NAME => WebSearchOutput::render_output(raw_json),
        write::NAME => WriteOutput::render_output(raw_json),
        _ => return raw_json.to_string(),
    };

    result.unwrap_or_else(|_| "No output".to_string())
}

pub const ALL_TOOL_NAMES: &[&str] = &[
    agent_tool::NAME,
    bash::NAME,
    batch::NAME,
    codesearch::NAME,
    edit::NAME,
    enter_worktree::NAME,
    exit_worktree::NAME,
    glob::NAME,
    grep::NAME,
    lsp::NAME,
    monitor::NAME,
    multiedit::NAME,
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

#[allow(clippy::too_many_arguments)]
pub fn create_tools(
    session_id: impl Into<String>,
    gctx: GlobalContext,
    tool_names: &[&str],
    model: Model<OpenAiWire>,
    parent_dir: impl AsRef<Path>,
    cwd: impl AsRef<Path>,
    lsp_client: Option<crate::lspclient::SharedLspClient>,
    extensions: AgentExtensions,
    notifier: Notifier,
    db: Db,
) -> ToolSet {
    let session_id = session_id.into();
    let parent_dir = parent_dir.as_ref();
    let cwd = cwd.as_ref();
    let tool_names = tool_names.iter().unique().collect_vec();
    let mut tool_set = ToolSet::default();

    let ctx = ToolContext::new(session_id, lsp_client.clone(), Notifier::clone(&notifier), cwd);

    for &tool_name in tool_names {
        match tool_name {
            agent_tool::NAME => {
                tool_set.add_tool(agent_tool::AgentTool::new(
                    gctx.clone(),
                    ctx.clone(),
                    model.clone(),
                    parent_dir,
                    Notifier::clone(&notifier),
                    extensions.clone(),
                    db.clone(),
                ));
            },
            bash::NAME => {
                tool_set.add_tool(bash::BashTool::new(ctx.clone()));
            },
            batch::NAME => {
                tool_set.add_tool(batch::BatchTool::new(
                    gctx.clone(),
                    ctx.clone(),
                    model.clone(),
                    parent_dir,
                    Notifier::clone(&notifier),
                    extensions.clone(),
                    db.clone(),
                ));
            },
            codesearch::NAME => {
                tool_set.add_tool(codesearch::CodeSearchTool::new());
            },
            edit::NAME => {
                tool_set.add_tool(edit::EditTool::new(ctx.clone()));
            },
            enter_worktree::NAME => {
                if let Some(repository) =
                    Repository::discover(cwd).ok().filter(|repo| repo.workdir().is_some())
                {
                    tool_set.add_tool(enter_worktree::EnterGitWorktreeTool::new(
                        ctx.clone(),
                        repository,
                        gctx.worktree_dir(),
                    ));
                }
            },
            exit_worktree::NAME => {
                if let Some(repository) =
                    Repository::discover(cwd).ok().filter(|repo| repo.workdir().is_some())
                {
                    tool_set
                        .add_tool(exit_worktree::ExitGitWorktreeTool::new(ctx.clone(), repository));
                }
            },
            glob::NAME => {
                tool_set.add_tool(glob::GlobTool::new(ctx.clone()));
            },
            grep::NAME => {
                tool_set.add_tool(grep::GrepTool::new(ctx.clone()));
            },
            lsp::NAME => {
                if let Some(lsp_client) = lsp_client.clone() {
                    tool_set.add_tool(lsp::LspTool::new(lsp_client, ctx.clone()));
                }
            },
            monitor::NAME => {
                tool_set.add_tool(monitor::MonitorTool::new(ctx.clone()));
            },
            multiedit::NAME => {
                tool_set.add_tool(multiedit::MultiEditTool::new(ctx.clone()));
            },
            read::NAME => {
                tool_set.add_tool(read::ReadTool::new(ctx.clone()));
            },
            skill::NAME => {
                if extensions.skills.is_empty() {
                    continue;
                }
                tool_set.add_tool(skill::SkillTool::new(extensions.skills.clone()));
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
                tool_set.add_tool(write::WriteTool::new(ctx.clone()));
            },
            _ => {},
        }
    }

    tool_set
}
