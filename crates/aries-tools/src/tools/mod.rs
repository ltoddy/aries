pub mod agent;
pub mod bash;
pub mod batch;
pub mod codesearch;
mod diff;
pub mod edit;
pub mod glob;
pub mod grep;
pub mod lsp;
pub mod monitor;
pub mod multiedit;
pub mod question;
pub mod read;
pub mod skill;
pub mod task_output;
pub mod task_stop;
pub mod update_plan;
pub mod webfetch;
pub mod websearch;
pub mod write;

use self::agent::AgentOutput;
use self::bash::BashOutput;
use self::batch::BatchOutput;
use self::codesearch::CodeSearchOutput;
use self::edit::EditOutput;
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
