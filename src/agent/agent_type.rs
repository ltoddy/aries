use rig::agent::Agent;
use rig::client::CompletionClient;

use crate::context::GlobalContext;
use crate::tools::{
    ApplyPatchTool, BatchTool, CodeSearchTool, EditTool, GlobTool, GrepTool, LsTool, LspTool, MultiEditTool,
    QuestionTool, ReadFileTool, ShellCommand, TaskTool, WebFetchTool, WebSearchTool, WriteFileTool,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum AgentType {
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
            AgentType::Build => include_str!("prompts/build.txt"),
            AgentType::Plan => include_str!("prompts/plan.txt"),
            AgentType::General => include_str!("prompts/generate.txt"),
            AgentType::Explore => include_str!("prompts/explore.txt"),
            AgentType::Compaction => include_str!("prompts/compaction.txt"),
            AgentType::Title => include_str!("prompts/title.txt"),
            AgentType::Summary => include_str!("prompts/summary.txt"),
        }
    }

    pub const fn max_turns(&self) -> usize {
        match self {
            AgentType::Build | AgentType::Plan | AgentType::General => 200,
            AgentType::Explore => 50,
            AgentType::Compaction | AgentType::Title | AgentType::Summary => 10,
        }
    }

    pub const fn temperature(&self) -> Option<f64> {
        match self {
            AgentType::Title => Some(0.5),
            _ => None,
        }
    }

    pub fn build_agent<M: rig::completion::CompletionModel>(
        &self,
        context: &GlobalContext,
        client: &impl CompletionClient<CompletionModel = M>,
        model: &str,
    ) -> Agent<M> {
        let preamble = self.system_prompt();
        let max_turns = self.max_turns();
        let temp = self.temperature();

        let builder = client.agent(model).preamble(preamble).default_max_turns(max_turns);

        match temp {
            Some(t) => self.build_with_tools_and_temp(context, builder, model, t),
            None => self.build_with_tools(context, builder, model),
        }
    }

    fn build_with_tools<M: rig::completion::CompletionModel>(
        &self,
        context: &GlobalContext,
        builder: rig::agent::AgentBuilder<M>,
        _model: &str,
    ) -> Agent<M> {
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
                .tool(BatchTool::new(context.clone()))
                .tool(QuestionTool)
                .tool(TaskTool::new(context.clone()))
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
            AgentType::Compaction | AgentType::Title | AgentType::Summary => builder.build(),
        }
    }

    fn build_with_tools_and_temp<M: rig::completion::CompletionModel>(
        &self,
        context: &GlobalContext,
        builder: rig::agent::AgentBuilder<M>,
        model: &str,
        temperature: f64,
    ) -> Agent<M> {
        match self {
            AgentType::Title => builder.temperature(temperature).build(),
            _ => self.build_with_tools(context, builder, model),
        }
    }
}
