mod agent;
mod command;
pub mod hook;
pub mod mcp;
mod skill;

use std::path::{Path, PathBuf};

pub use self::agent::{AgentDefinition, AgentsLoader, Frontmatter as AgentFrontmatter};
pub use self::command::{CommandDefinition, CommandsLoader, Frontmatter as CommandFrontmatter};
pub use self::hook::{HookDecision, HooksDefinition, HooksExecutor, HooksLoader};
pub use self::mcp::{
    Http, McpConnectError, McpDefinition, McpLoadResult, McpParseError, McpServerConfig,
    McpsLoader, Sse, Stdio, connect,
};
pub use self::skill::{Frontmatter as SkillFrontmatter, SkillDefinition, SkillsLoader};

#[derive(Debug, Clone, Default)]
pub struct AgentExtensions {
    pub agents: Vec<AgentDefinition>,
    pub commands: Vec<CommandDefinition>,
    pub hooks: Vec<HooksDefinition>,
    pub mcps: Vec<McpDefinition>,
    pub skills: Vec<SkillDefinition>,
}

impl AgentExtensions {
    pub async fn new(cwd: impl AsRef<Path>) -> Self {
        let cwd = cwd.as_ref();
        let home_dir = std::env::home_dir().unwrap_or_else(|| PathBuf::from("~"));

        let agent_loader = AgentsLoader::new(cwd, &home_dir);
        let command_loader = CommandsLoader::new(cwd, &home_dir);
        let hook_loader = HooksLoader::new(cwd, &home_dir);
        let mcp_loader = McpsLoader::new(cwd, &home_dir);
        let skill_loader = SkillsLoader::new(cwd, &home_dir);

        let (agents, commands, hooks, mcps, skills) = tokio::join!(
            agent_loader.load(),
            command_loader.load(),
            hook_loader.load(),
            mcp_loader.load(),
            skill_loader.load(),
        );

        Self { agents, commands, hooks, mcps, skills }
    }

    pub fn empty() -> Self {
        Self::default()
    }
}
