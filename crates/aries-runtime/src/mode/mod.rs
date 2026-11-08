use std::fmt::{Display, Formatter};
use std::str::FromStr;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Build,
    General,
    Explore,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseModeError;

impl Display for ParseModeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        "provieded string was not `build`, `general` or `explore`".fmt(f)
    }
}

impl std::error::Error for ParseModeError {}

impl FromStr for Mode {
    type Err = ParseModeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "build" => Ok(Self::Build),
            "general" => Ok(Self::General),
            "explore" => Ok(Self::Explore),
            _ => Err(ParseModeError),
        }
    }
}

impl From<Mode> for String {
    fn from(val: Mode) -> Self {
        match val {
            Mode::Build => String::from("build"),
            Mode::General => String::from("general"),
            Mode::Explore => String::from("explore"),
        }
    }
}

impl Display for Mode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Mode::Build => write!(f, "build"),
            Mode::General => write!(f, "general"),
            Mode::Explore => write!(f, "explore"),
        }
    }
}

impl Mode {
    pub const fn bare_preamble(self) -> &'static str {
        match self {
            Self::Build => include_str!("prompts/build.md"),
            Self::General => include_str!("prompts/general.md"),
            Self::Explore => include_str!("prompts/explore.md"),
        }
    }

    pub const fn id(self) -> &'static str {
        match self {
            Self::Build => "build",
            Self::General => "general",
            Self::Explore => "explore",
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Build => "Builder",
            Self::General => "General",
            Self::Explore => "Explorer",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::Build => "默认主智能体。直接使用工具执行任务，并在需要时委托子智能体。",
            Self::General => {
                "用于研究复杂问题和执行多步任务的通用智能体。使用此智能体并行执行多个工作单元。"
            },
            Self::Explore => {
                "专门用于探索代码库的快速智能体。当您需要通过模式快速查找文件、搜索关键字或回答有关代码库的问题时使用。"
            },
        }
    }
}
