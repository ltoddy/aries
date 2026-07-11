pub mod agent;

use std::path::PathBuf;

use aries_extension::skill::SkillDefinition;
use aries_tools::bash::{BashArgs, BashOutput, BashTool};
use aries_tools::batch::{BatchArgs, BatchOutput, BatchTool};
use aries_tools::codesearch::{CodeSearchArgs, CodeSearchOutput, CodeSearchTool};
use aries_tools::edit::{EditArgs, EditOutput, EditTool};
use aries_tools::glob::{GlobArgs, GlobOutput, GlobTool};
use aries_tools::grep::{GrepArgs, GrepOutput, GrepTool};
use aries_tools::ls::{LsArgs, LsOutput, LsTool};
use aries_tools::lsp::{LspArgs, LspOutput, LspTool};
use aries_tools::multiedit::{MultiEditArgs, MultiEditOutput, MultiEditTool};
use aries_tools::question::{AskUserQuestionArgs, AskUserQuestionOutput, AskUserQuestionTool};
use aries_tools::read::{ReadArgs, ReadOutput, ReadTool};
use aries_tools::skill::{SkillArgs, SkillOutput, SkillTool};
use aries_tools::update_plan::{UpdatePlanArgs, UpdatePlanOutput, UpdatePlanTool};
use aries_tools::webfetch::{WebFetchArgs, WebFetchOutput, WebFetchTool};
use aries_tools::websearch::{WebSearchArgs, WebSearchOutput, WebSearchTool};
use aries_tools::write::{WriteArgs, WriteOutput, WriteTool};
use aries_tools::{
    bash, batch, codesearch, edit, glob, grep, ls, lsp, multiedit, question, read, skill,
    update_plan, webfetch, websearch, write,
};
use rig_core::tool::Tool;
use serde_json::Value;

pub use crate::tools::agent::{AgentArgs, AgentOutput, AgentTool};

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
    skills: &[SkillDefinition],
) -> Vec<Box<dyn rig_core::tool::ToolDyn>> {
    let cwd = cwd.to_path_buf();
    let mut tools: Vec<Box<dyn rig_core::tool::ToolDyn>> = Vec::with_capacity(tool_names.len());

    for &name in tool_names {
        let tool: Box<dyn rig_core::tool::ToolDyn> = match name {
            agent::NAME => continue, // generic over client, skip
            bash::NAME => Box::new(BashTool),
            batch::NAME => Box::new(BatchTool::new(cwd.clone(), move |tool_name, params, cwd| {
                Box::pin(dispatch_batch_call(tool_name, params, cwd))
            })),
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
                if skills.is_empty() {
                    continue;
                }
                Box::new(SkillTool::new(skills.to_vec()))
            },
            update_plan::NAME => {
                let sender = sender.clone();
                Box::new(UpdatePlanTool::new(move |entries| {
                    let event = crate::event::AgentEvent::from_plan(true, "main", entries);
                    sender.send(event).map_err(|_| {
                        aries_tools::update_plan::UpdatePlanError::SendFailed(
                            "receiver dropped".to_owned(),
                        )
                    })
                }))
            },
            webfetch::NAME => Box::new(WebFetchTool),
            websearch::NAME => Box::new(WebSearchTool),
            write::NAME => Box::new(WriteTool),
            _ => continue,
        };
        tools.push(tool);
    }

    tools
}

async fn dispatch_batch_call(
    tool_name: String,
    params: Value,
    cwd: PathBuf,
) -> Result<Value, String> {
    match tool_name.as_str() {
        bash::NAME => {
            let args: BashArgs = serde_json::from_value(params).map_err(|e| e.to_string())?;
            Tool::call(&BashTool, args)
                .await
                .map(|res| serde_json::to_value(res).unwrap())
                .map_err(|e| e.to_string())
        },
        read::NAME => {
            let args: ReadArgs = serde_json::from_value(params).map_err(|e| e.to_string())?;
            Tool::call(&ReadTool, args)
                .await
                .map(|res| serde_json::to_value(res).unwrap())
                .map_err(|e| e.to_string())
        },
        write::NAME => {
            let args: WriteArgs = serde_json::from_value(params).map_err(|e| e.to_string())?;
            Tool::call(&WriteTool, args)
                .await
                .map(|res| serde_json::to_value(res).unwrap())
                .map_err(|e| e.to_string())
        },
        glob::NAME => {
            let args: GlobArgs = serde_json::from_value(params).map_err(|e| e.to_string())?;
            Tool::call(&GlobTool::new(cwd), args)
                .await
                .map(|res| serde_json::to_value(res).unwrap())
                .map_err(|e| e.to_string())
        },
        grep::NAME => {
            let args: GrepArgs = serde_json::from_value(params).map_err(|e| e.to_string())?;
            Tool::call(&GrepTool::new(cwd), args)
                .await
                .map(|res| serde_json::to_value(res).unwrap())
                .map_err(|e| e.to_string())
        },
        ls::NAME => {
            let args: LsArgs = serde_json::from_value(params).map_err(|e| e.to_string())?;
            Tool::call(&LsTool::new(cwd), args)
                .await
                .map(|res| serde_json::to_value(res).unwrap())
                .map_err(|e| e.to_string())
        },
        multiedit::NAME => {
            let args: MultiEditArgs = serde_json::from_value(params).map_err(|e| e.to_string())?;
            Tool::call(&MultiEditTool, args)
                .await
                .map(|res| serde_json::to_value(res).unwrap())
                .map_err(|e| e.to_string())
        },
        edit::NAME => {
            let args: EditArgs = serde_json::from_value(params).map_err(|e| e.to_string())?;
            Tool::call(&EditTool, args)
                .await
                .map(|res| serde_json::to_value(res).unwrap())
                .map_err(|e| e.to_string())
        },
        question::NAME => {
            let args: AskUserQuestionArgs =
                serde_json::from_value(params).map_err(|e| e.to_string())?;
            Tool::call(&AskUserQuestionTool, args)
                .await
                .map(|res| serde_json::to_value(res).unwrap())
                .map_err(|e| e.to_string())
        },
        agent::NAME => Err("AgentTool is not allowed in batch".to_string()),
        webfetch::NAME => {
            let args: WebFetchArgs = serde_json::from_value(params).map_err(|e| e.to_string())?;
            Tool::call(&WebFetchTool, args)
                .await
                .map(|res| serde_json::to_value(res).unwrap())
                .map_err(|e| e.to_string())
        },
        websearch::NAME => {
            let args: WebSearchArgs = serde_json::from_value(params).map_err(|e| e.to_string())?;
            Tool::call(&WebSearchTool, args)
                .await
                .map(|res| serde_json::to_value(res).unwrap())
                .map_err(|e| e.to_string())
        },
        codesearch::NAME => {
            let args: CodeSearchArgs = serde_json::from_value(params).map_err(|e| e.to_string())?;
            Tool::call(&CodeSearchTool, args)
                .await
                .map(|res| serde_json::to_value(res).unwrap())
                .map_err(|e| e.to_string())
        },
        _ => Err(format!("Tool '{}' not found or not supported in batch", tool_name)),
    }
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

pub use aries_tools::{RenderError, ToolArgsRender, ToolOutputRender};
