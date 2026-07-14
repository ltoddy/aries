use aries_tools::agent::AgentOutput;
use aries_tools::bash::BashOutput;
use aries_tools::batch::BatchOutput;
use aries_tools::codesearch::CodeSearchOutput;
use aries_tools::edit::EditOutput;
use aries_tools::glob::GlobOutput;
use aries_tools::grep::GrepOutput;
use aries_tools::ls::LsOutput;
use aries_tools::lsp::LspOutput;
use aries_tools::multiedit::MultiEditOutput;
use aries_tools::question::AskUserQuestionOutput;
use aries_tools::read::ReadOutput;
use aries_tools::skill::SkillOutput;
use aries_tools::update_plan::UpdatePlanOutput;
use aries_tools::webfetch::WebFetchOutput;
use aries_tools::websearch::WebSearchOutput;
use aries_tools::write::WriteOutput;
use aries_tools::{
    ToolOutputRender, agent, bash, batch, codesearch, edit, glob, grep, ls, lsp, multiedit,
    question, read, skill, update_plan, webfetch, websearch, write,
};

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
