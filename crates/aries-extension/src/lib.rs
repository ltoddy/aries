pub mod agent;
pub mod command;
pub mod hook;
pub mod mcp;
pub mod skill;
mod tool;
pub mod uri;

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct AgentExtensions {
    pub agents: Vec<agent::AgentDefinition>,
    pub commands: Vec<command::CommandDefinition>,
    pub hooks: Vec<hook::HooksDefinition>,
    pub mcps: Vec<mcp::McpDefinition>,
    pub skills: Vec<skill::SkillDefinition>,
}

impl AgentExtensions {
    pub async fn new(cwd: impl AsRef<Path>, root_dir: impl AsRef<Path>) -> Self {
        let cwd = cwd.as_ref();
        let root_dir = root_dir.as_ref();
        let home_dir = std::env::home_dir().unwrap_or_else(|| PathBuf::from("~"));

        let agent_loader = agent::AgentsLoader::new(cwd, &home_dir, root_dir);
        let command_loader = command::CommandsLoader::new(cwd, &home_dir);
        let hook_loader = hook::HooksLoader::new(cwd, &home_dir);
        let mcp_loader = mcp::McpsLoader::new(cwd, &home_dir);
        let skill_loader = skill::SkillsLoader::new(cwd, &home_dir, root_dir);

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

    pub fn available_commands(&self) -> Vec<AvailableCommand> {
        let mut available_commands = Vec::with_capacity(self.commands.len() + self.skills.len());

        for c in &self.commands {
            available_commands.push(c.into());
        }
        for s in &self.skills {
            available_commands.push(s.into());
        }

        available_commands
    }
}

#[derive(Debug, Clone)]
pub struct AvailableCommand {
    pub name: String,
    pub description: String,
    pub argument_hint: Option<String>,
}

impl AvailableCommand {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        argument_hint: Option<String>,
    ) -> Self {
        let name = name.into();
        let description = description.into();

        Self { name, description, argument_hint }
    }
}

impl From<&skill::SkillDefinition> for AvailableCommand {
    fn from(value: &skill::SkillDefinition) -> Self {
        AvailableCommand::new(&value.frontmatter.name, &value.frontmatter.description, None)
    }
}

impl From<&command::CommandDefinition> for AvailableCommand {
    fn from(value: &command::CommandDefinition) -> Self {
        AvailableCommand::new(
            &value.frontmatter.name,
            &value.frontmatter.description,
            value.frontmatter.argument_hint.clone(),
        )
    }
}
