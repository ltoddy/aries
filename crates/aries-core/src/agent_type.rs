use aries_context::GlobalContext;
use rig::tool::ToolDyn;

use crate::tools::{
    ApplyPatchTool, BatchTool, CodeSearchTool, EditTool, GlobTool, GrepTool, LsTool, LspTool, MultiEditTool,
    QuestionTool, ReadFileTool, ShellCommand, TaskTool, WebFetchTool, WebSearchTool, WriteFileTool,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum AgentType {
    Orchestrate,
    Build,
    Plan,
    General,
    Explore,
    Compaction,
    Title,
    Summary,
}

#[allow(dead_code)]
impl AgentType {
    pub const fn name(&self) -> &'static str {
        match self {
            AgentType::Orchestrate => "orchestrate",
            AgentType::Build => "build",
            AgentType::Plan => "plan",
            AgentType::General => "general",
            AgentType::Explore => "explore",
            AgentType::Compaction => "compaction",
            AgentType::Title => "title",
            AgentType::Summary => "summary",
        }
    }

    pub const fn description(&self) -> &'static str {
        match self {
            AgentType::Orchestrate => "编排智能体。负责任务分解和委托，不直接执行操作。",
            AgentType::Build => "默认智能体。根据配置的权限执行工具。",
            AgentType::Plan => "计划模式。不允许使用所有编辑工具。",
            AgentType::General => "用于研究复杂问题和执行多步任务的通用智能体。使用此智能体并行执行多个工作单元。",
            AgentType::Explore => {
                "专门用于探索代码库的快速智能体。当您需要通过模式快速查找文件、搜索关键字或回答有关代码库的问题时使用。"
            },
            AgentType::Compaction => "用于压缩和总结对话上下文的智能体。",
            AgentType::Title => "用于生成对话标题的智能体。",
            AgentType::Summary => "用于生成对话摘要（类似于 PR 描述）的智能体。",
        }
    }

    pub const fn system_prompt(&self) -> &'static str {
        match self {
            AgentType::Orchestrate => include_str!("prompts/orchestrate.txt"),
            AgentType::Build => include_str!("prompts/build.txt"),
            AgentType::Plan => include_str!("prompts/plan.txt"),
            AgentType::General => include_str!("prompts/generate.txt"),
            AgentType::Explore => include_str!("prompts/explore.txt"),
            AgentType::Compaction => include_str!("prompts/compaction.txt"),
            AgentType::Title => include_str!("prompts/title.txt"),
            AgentType::Summary => include_str!("prompts/summary.txt"),
        }
    }

    pub fn tools(&self, context: GlobalContext) -> Vec<Box<dyn ToolDyn>> {
        match self {
            AgentType::Orchestrate => vec![Box::new(QuestionTool), Box::new(TaskTool::new(context))],
            AgentType::Build | AgentType::General => vec![
                Box::new(ShellCommand),
                Box::new(ReadFileTool),
                Box::new(WriteFileTool),
                Box::new(GlobTool),
                Box::new(GrepTool),
                Box::new(LsTool),
                Box::new(ApplyPatchTool),
                Box::new(MultiEditTool),
                Box::new(EditTool),
                Box::new(BatchTool::new(context.clone())),
                Box::new(QuestionTool),
                Box::new(TaskTool::new(context)),
                Box::new(WebFetchTool),
                Box::new(WebSearchTool),
                Box::new(LspTool),
                Box::new(CodeSearchTool),
            ],
            AgentType::Plan => vec![
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
            ],
            AgentType::Explore => vec![
                Box::new(ReadFileTool),
                Box::new(GlobTool),
                Box::new(GrepTool),
                Box::new(LsTool),
                Box::new(WebFetchTool),
                Box::new(WebSearchTool),
                Box::new(CodeSearchTool),
            ],
            AgentType::Compaction | AgentType::Title | AgentType::Summary => vec![],
        }
    }
}
