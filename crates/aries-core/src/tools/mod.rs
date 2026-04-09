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

pub use self::apply_patch::{ApplyPatchArgs, ApplyPatchOutput, ApplyPatchTool};
pub use self::bash::{ShellCommand, ShellCommandArgs, ShellCommandOutput};
pub use self::batch::*;
pub use self::codesearch::{CodeSearchArgs, CodeSearchOutput, CodeSearchTool};
pub use self::edit::{EditArgs, EditOutput, EditTool};
pub use self::glob::{GlobArgs, GlobOutput, GlobTool};
pub use self::grep::{GrepArgs, GrepOutput, GrepTool};
pub use self::ls::{LsArgs, LsOutput, LsTool};
pub use self::lsp::{LspArgs, LspOutput, LspTool};
pub use self::multiedit::{MultiEditArgs, MultiEditOutput, MultiEditTool};
pub use self::question::{QuestionArgs, QuestionOutput, QuestionTool};
pub use self::read::{ReadFileArgs, ReadFileOutput, ReadFileTool};
pub use self::task::{TaskArgs, TaskOutput, TaskTool};
pub use self::webfetch::{WebFetchArgs, WebFetchOutput, WebFetchTool};
pub use self::websearch::{WebSearchArgs, WebSearchOutput, WebSearchTool};
pub use self::write::{WriteFileArgs, WriteFileOutput, WriteFileTool};

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
        Box::new(LspTool),
        Box::new(CodeSearchTool),
        // Box::new(WebFetchTool),
        // Box::new(WebSearchTool),
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
        // Box::new(WebFetchTool),
        // Box::new(WebSearchTool),
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
        Box::new(LspTool),
        Box::new(CodeSearchTool),
        // Box::new(WebFetchTool),
        // Box::new(WebSearchTool),
    ]
}

pub fn explore_tools() -> Vec<Box<dyn ToolDyn>> {
    vec![
        Box::new(ShellCommand),
        Box::new(ReadFileTool),
        Box::new(GlobTool),
        Box::new(GrepTool),
        Box::new(LsTool),
        Box::new(CodeSearchTool),
        // Box::new(WebFetchTool),
        // Box::new(WebSearchTool),
    ]
}
