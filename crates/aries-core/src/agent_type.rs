#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentType {
    Build,
    Plan,
    General,
    Explore,
    Title,
    Summary,
}

impl AgentType {
    pub const fn bare_preamble(self) -> &'static str {
        match self {
            Self::Build => include_str!("prompts/build.txt"),
            Self::Plan => include_str!("prompts/plan.txt"),
            Self::General => include_str!("prompts/generate.txt"),
            Self::Explore => include_str!("prompts/explore.txt"),
            Self::Title => include_str!("prompts/title.txt"),
            Self::Summary => include_str!("prompts/summary.txt"),
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Build => "Builder",
            Self::Plan => "Planner",
            Self::General => "Assistant",
            Self::Explore => "Explorer",
            Self::Title => "Namer",
            Self::Summary => "Summarizer",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::Build => "默认主智能体。直接使用工具执行任务，并在需要时委托子智能体。",
            Self::Plan => "计划模式。不允许使用所有编辑工具。",
            Self::General => {
                "用于研究复杂问题和执行多步任务的通用智能体。使用此智能体并行执行多个工作单元。"
            },
            Self::Explore => {
                "专门用于探索代码库的快速智能体。当您需要通过模式快速查找文件、搜索关键字或回答有关代码库的问题时使用。"
            },
            Self::Title => "用于生成对话标题的智能体。",
            Self::Summary => "用于生成对话摘要（类似于 PR 描述）的智能体。",
        }
    }
}
