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

pub fn format_tool_output(tool_name: &str, raw_json: serde_json::Value) -> String {
    let result = match tool_name {
        agent::NAME => agent::AgentOutput::render_output(raw_json),
        bash::NAME => bash::BashOutput::render_output(raw_json),
        batch::NAME => batch::BatchOutput::render_output(raw_json),
        codesearch::NAME => codesearch::CodeSearchOutput::render_output(raw_json),
        edit::NAME => edit::EditOutput::render_output(raw_json),
        enter_worktree::NAME => enter_worktree::EnterGitWorktreeOutput::render_output(raw_json),
        exit_worktree::NAME => exit_worktree::ExitGitWorktreeOutput::render_output(raw_json),
        glob::NAME => glob::GlobOutput::render_output(raw_json),
        grep::NAME => grep::GrepOutput::render_output(raw_json),
        monitor::NAME => monitor::MonitorOutput::render_output(raw_json),
        multiedit::NAME => multiedit::MultiEditOutput::render_output(raw_json),
        read::NAME => read::ReadOutput::render_output(raw_json),
        skill::NAME => skill::SkillOutput::render_output(raw_json),
        task_output::NAME => task_output::TaskOutputOutput::render_output(raw_json),
        task_stop::NAME => task_stop::TaskStopOutput::render_output(raw_json),
        update_plan::NAME => update_plan::UpdatePlanOutput::render_output(raw_json),
        webfetch::NAME => webfetch::WebFetchOutput::render_output(raw_json),
        websearch::NAME => websearch::WebSearchOutput::render_output(raw_json),
        write::NAME => write::WriteOutput::render_output(raw_json),
        _ => return raw_json.to_string(),
    };

    result.unwrap_or_else(|_| "No output".to_string())
}

pub const ALL_TOOL_NAMES: &[&str] = &[
    agent::NAME,
    bash::NAME,
    batch::NAME,
    codesearch::NAME,
    edit::NAME,
    enter_worktree::NAME,
    exit_worktree::NAME,
    glob::NAME,
    grep::NAME,
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
    extensions: AgentExtensions,
    notifier: Notifier,
    db: Db,
) -> ToolSet {
    let session_id = session_id.into();
    let parent_dir = parent_dir.as_ref();
    let cwd = cwd.as_ref();
    let tool_names = tool_names.iter().unique().collect_vec();
    let mut tool_set = ToolSet::default();

    let ctx = context::ToolContext::new(session_id, Notifier::clone(&notifier), cwd);

    for &tool_name in tool_names {
        match tool_name {
            agent::NAME => {
                tool_set.add_tool(agent::AgentTool::new(
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
                        gctx.worktree_dir().as_ref(),
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
