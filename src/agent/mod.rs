pub mod runner;

use rig::agent::Agent;
use rig::client::CompletionClient;
use rig::providers::deepseek;

use crate::tools::{
    ApplyPatchTool, CodeSearchTool, EditTool, GlobTool, GrepTool, LsTool, LspTool, MultiEditTool, QuestionTool,
    ReadFileTool, ShellCommand, TaskTool, WebFetchTool, WebSearchTool, WriteFileTool,
};

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentType {
    Build,
    Plan,
    General,
    Explore,
    Compaction,
    Title,
    Summary,
}

impl AgentType {
    #[allow(dead_code)]
    pub fn name(&self) -> &'static str {
        match self {
            AgentType::Build => "build",
            AgentType::Plan => "plan",
            AgentType::General => "general",
            AgentType::Explore => "explore",
            AgentType::Compaction => "compaction",
            AgentType::Title => "title",
            AgentType::Summary => "summary",
        }
    }

    #[allow(dead_code)]
    pub fn description(&self) -> &'static str {
        match self {
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

    pub fn prompt(&self) -> &'static str {
        match self {
            AgentType::Build => {
                "You are Aries, a helpful terminal AI assistant. You can use tools to execute shell commands and read/write files when requested by the user. Always explain what you are going to do before calling a tool."
            },
            AgentType::Plan => {
                "You are a planning agent. You can explore and read, but you cannot edit. Create a plan and ask the user for approval."
            },
            AgentType::General => include_str!("prompts/generate.txt"),
            AgentType::Explore => include_str!("prompts/explore.txt"),
            AgentType::Compaction => include_str!("prompts/compaction.txt"),
            AgentType::Title => include_str!("prompts/title.txt"),
            AgentType::Summary => include_str!("prompts/summary.txt"),
        }
    }

    pub fn build_agent(&self, client: &deepseek::Client, model: &str) -> Agent<deepseek::CompletionModel> {
        let builder = client.agent(model).preamble(self.prompt()).default_max_turns(200);

        match self {
            AgentType::Build | AgentType::General => builder
                .tool(ShellCommand)
                .tool(ReadFileTool)
                .tool(WriteFileTool)
                .tool(GlobTool)
                .tool(GrepTool)
                .tool(LsTool)
                .tool(ApplyPatchTool)
                .tool(MultiEditTool)
                .tool(EditTool)
                .tool(QuestionTool)
                .tool(TaskTool)
                .tool(WebFetchTool)
                .tool(WebSearchTool)
                .tool(LspTool)
                .tool(CodeSearchTool)
                .build(),
            AgentType::Plan => builder
                .tool(ShellCommand)
                .tool(ReadFileTool)
                .tool(GlobTool)
                .tool(GrepTool)
                .tool(LsTool)
                .tool(QuestionTool)
                .tool(WebFetchTool)
                .tool(WebSearchTool)
                .tool(LspTool)
                .tool(CodeSearchTool)
                .build(),
            AgentType::Explore => builder
                .tool(ReadFileTool)
                .tool(GlobTool)
                .tool(GrepTool)
                .tool(LsTool)
                .tool(WebFetchTool)
                .tool(WebSearchTool)
                .tool(CodeSearchTool)
                .build(),
            AgentType::Compaction | AgentType::Title | AgentType::Summary => {
                // Pure LLM tasks
                builder.build()
            },
        }
    }
}
