pub mod agent;
pub mod command;
pub mod hook;
pub mod mcp;
pub mod skill;

use std::path::{Path, PathBuf};

use crate::agent::{AgentDefinition, AgentsLoader};
use crate::command::{CommandDefinition, CommandsLoader};
use crate::hook::{HooksDefinition, HooksLoader};
use crate::mcp::{McpDefinition, McpsLoader};
use crate::skill::{SkillDefinition, SkillsLoader};

#[derive(Clone, Default)]
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
