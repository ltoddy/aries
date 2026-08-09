use aries_extension::command::CommandDefinition;
use rig_core::message::Message;

use crate::AriesAgentProvider;

pub struct SlashCommandsExecutor<'a> {
    agent: &'a AriesAgentProvider,
    commands: &'a [CommandDefinition],
}

impl<'a> SlashCommandsExecutor<'a> {
    pub fn new(agent: &'a AriesAgentProvider, commands: &'a [CommandDefinition]) -> Self {
        Self { agent, commands }
    }

    pub async fn execute(&self, command: impl AsRef<str>, args: impl AsRef<str>) -> bool {
        let command = command.as_ref();
        let args = args.as_ref();

        let Some(command) = self.commands.iter().find(|c| c.frontmatter.name == command) else {
            return false;
        };

        let prompt = command.expand_arguments(args);
        let _ = self.agent.prompt::<_, Message, _>(prompt, [], ()).await;
        true
    }
}
