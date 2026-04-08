pub mod apply_patch;
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
pub mod task;
pub mod webfetch;
pub mod websearch;
pub mod write;

use aries_config::AriesConfig;
use rig::agent::PromptHook;
use rig::providers::{azure, openai};
use rig::tool::ToolDyn;

pub use self::apply_patch::{ApplyPatchOutput, ApplyPatchTool};
pub use self::bash::{ShellCommand, ShellCommandOutput};
pub use self::batch::*;
pub use self::codesearch::{CodeSearchOutput, CodeSearchTool};
pub use self::edit::{EditOutput, EditTool};
pub use self::glob::{GlobOutput, GlobTool};
pub use self::grep::{GrepOutput, GrepTool};
pub use self::ls::{LsOutput, LsTool};
pub use self::lsp::{LspOutput, LspTool};
pub use self::multiedit::{MultiEditOutput, MultiEditTool};
pub use self::question::{QuestionOutput, QuestionTool};
pub use self::read::{ReadFileOutput, ReadFileTool};
pub use self::task::{TaskOutput, TaskTool};
pub use self::webfetch::{WebFetchOutput, WebFetchTool};
pub use self::websearch::{WebSearchOutput, WebSearchTool};
pub use self::write::{WriteFileOutput, WriteFileTool};

pub fn build_openai_tools<P>(config: AriesConfig, hook: P) -> Vec<Box<dyn ToolDyn>>
where
    P: PromptHook<openai::CompletionModel> + 'static,
{
    vec![
        Box::new(ShellCommand),
        Box::new(ReadFileTool),
        Box::new(WriteFileTool),
        Box::new(GlobTool),
        Box::new(GrepTool),
        Box::new(LsTool),
        Box::new(ApplyPatchTool),
        Box::new(MultiEditTool),
        Box::new(EditTool),
        Box::new(BatchTool::<openai::CompletionModel, P>::new(config.clone(), hook.clone())),
        Box::new(QuestionTool),
        Box::new(TaskTool::<openai::CompletionModel, P>::new(config, hook)),
        Box::new(WebFetchTool),
        Box::new(WebSearchTool),
        Box::new(LspTool),
        Box::new(CodeSearchTool),
    ]
}

pub fn build_azure_tools<P>(config: AriesConfig, hook: P) -> Vec<Box<dyn ToolDyn>>
where
    P: PromptHook<azure::CompletionModel> + 'static,
{
    vec![
        Box::new(ShellCommand),
        Box::new(ReadFileTool),
        Box::new(WriteFileTool),
        Box::new(GlobTool),
        Box::new(GrepTool),
        Box::new(LsTool),
        Box::new(ApplyPatchTool),
        Box::new(MultiEditTool),
        Box::new(EditTool),
        Box::new(BatchTool::<azure::CompletionModel, P>::new(config.clone(), hook.clone())),
        Box::new(QuestionTool),
        Box::new(TaskTool::<azure::CompletionModel, P>::new(config, hook)),
        Box::new(WebFetchTool),
        Box::new(WebSearchTool),
        Box::new(LspTool),
        Box::new(CodeSearchTool),
    ]
}

pub fn plan_tools() -> Vec<Box<dyn ToolDyn>> {
    vec![
        Box::new(ShellCommand),
        Box::new(ReadFileTool),
        Box::new(GlobTool),
        Box::new(GrepTool),
        Box::new(LsTool),
        Box::new(QuestionTool),
        Box::new(WebFetchTool),
        Box::new(WebSearchTool),
        Box::new(LspTool),
        Box::new(CodeSearchTool),
    ]
}

pub fn explore_tools() -> Vec<Box<dyn ToolDyn>> {
    vec![
        Box::new(ShellCommand),
        Box::new(ReadFileTool),
        Box::new(GlobTool),
        Box::new(GrepTool),
        Box::new(LsTool),
        Box::new(WebFetchTool),
        Box::new(WebSearchTool),
        Box::new(CodeSearchTool),
    ]
}
