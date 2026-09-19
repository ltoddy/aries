mod agent;
mod command;
pub mod hook;
pub mod mcp;
mod skill;
mod tool;

use std::path::{Path, PathBuf};

pub use self::agent::{AgentDefinition, AgentsLoader, Frontmatter as AgentFrontmatter};
pub use self::command::{
    CommandDefinition, CommandsLoader, Frontmatter as CommandFrontmatter, SlashCommandsExecutor,
};
pub use self::hook::{HookDecision, HooksDefinition, HooksExecutor, HooksLoader};
pub use self::mcp::{
    Http, McpConnectError, McpDefinition, McpLoadResult, McpParseError, McpServerConfig,
    McpsLoader, Sse, Stdio, connect,
};
pub use self::skill::{
    Frontmatter as SkillFrontmatter, SkillDefinition, SkillExecutor, SkillsLoader,
};

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

impl From<&SkillDefinition> for AvailableCommand {
    fn from(value: &SkillDefinition) -> Self {
        AvailableCommand::new(&value.frontmatter.name, &value.frontmatter.description, None)
    }
}

impl From<&CommandDefinition> for AvailableCommand {
    fn from(value: &CommandDefinition) -> Self {
        AvailableCommand::new(
            &value.frontmatter.name,
            &value.frontmatter.description,
            value.frontmatter.argument_hint.clone(),
        )
    }
}
