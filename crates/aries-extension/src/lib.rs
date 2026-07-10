pub mod agent;
pub mod hook;
pub mod mcp;
pub mod skill;

use std::path::Path;

use crate::agent::{CustomAgentDefinition, CustomAgentsLoader};
use crate::hook::{HooksLoader, HooksPreset};
use crate::mcp::{McpDefinition, McpsLoader};
use crate::skill::{SkillDefinition, SkillsLoader};

#[derive(Clone)]
pub struct AgentExtensions {
    pub agents: Vec<CustomAgentDefinition>,
    pub hooks: Vec<HooksPreset>,
    pub mcps: Vec<McpDefinition>,
    pub skills: Vec<SkillDefinition>,
}

impl AgentExtensions {
    pub async fn new(cwd: impl AsRef<Path>) -> Self {
        let cwd = cwd.as_ref();

        let agent_loader = CustomAgentsLoader::new(cwd);
        let hook_loader = HooksLoader::new(cwd);
        let mcp_loader = McpsLoader::new(cwd);
        let skill_loader = SkillsLoader::new(cwd);

        let (agents, hooks, mcps, skills) = tokio::join!(
            agent_loader.load(),
            hook_loader.load(),
            mcp_loader.load(),
            skill_loader.load(),
        );

        Self { agents, hooks, mcps, skills }
    }

    pub fn empty() -> Self {
        Self { agents: vec![], hooks: vec![], mcps: vec![], skills: vec![] }
    }
}
