#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentType {
    Build,
    Plan,
    General,
    Explore,
}

impl AgentType {
    pub fn from_id(id: &str) -> Self {
        match id {
            "build" => Self::Build,
            "plan" => Self::Plan,
            "general" => Self::General,
            "explore" => Self::Explore,
            _ => Self::Build,
        }
    }

    pub const fn bare_preamble(self) -> &'static str {
        match self {
            Self::Build => include_str!("prompts/build.txt"),
            Self::Plan => include_str!("prompts/plan.txt"),
            Self::General => include_str!("prompts/generate.txt"),
            Self::Explore => include_str!("prompts/explore.txt"),
        }
    }

    pub const fn id(self) -> &'static str {
        match self {
            Self::Build => "build",
            Self::Plan => "plan",
            Self::General => "general",
            Self::Explore => "explore",
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Build => "Builder",
            Self::Plan => "Planner",
            Self::General => "Assistant",
            Self::Explore => "Explorer",
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
        }
    }
}
