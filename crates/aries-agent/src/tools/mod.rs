use aries_tools::agent::{AgentArgs, AgentOutput};
use aries_tools::bash::{BashArgs, BashOutput};
use aries_tools::batch::{BatchArgs, BatchOutput};
use aries_tools::codesearch::{CodeSearchArgs, CodeSearchOutput};
use aries_tools::edit::{EditArgs, EditOutput};
use aries_tools::glob::{GlobArgs, GlobOutput};
use aries_tools::grep::{GrepArgs, GrepOutput};
use aries_tools::ls::{LsArgs, LsOutput};
use aries_tools::lsp::{LspArgs, LspOutput};
use aries_tools::multiedit::{MultiEditArgs, MultiEditOutput};
use aries_tools::question::{AskUserQuestionArgs, AskUserQuestionOutput};
use aries_tools::read::{ReadArgs, ReadOutput};
use aries_tools::skill::{SkillArgs, SkillOutput};
use aries_tools::update_plan::{UpdatePlanArgs, UpdatePlanOutput};
use aries_tools::webfetch::{WebFetchArgs, WebFetchOutput};
use aries_tools::websearch::{WebSearchArgs, WebSearchOutput};
use aries_tools::write::{WriteArgs, WriteOutput};
use aries_tools::{
    ToolArgsRender, ToolOutputRender, agent, bash, batch, codesearch, edit, glob, grep, ls, lsp,
    multiedit, question, read, skill, update_plan, webfetch, websearch, write,
};

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
