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

    pub const fn preamble(&self) -> &'static str {
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
}
