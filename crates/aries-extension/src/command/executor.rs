use crate::command::CommandDefinition;

#[derive(Debug)]
pub struct SlashCommandsExecutor<'a> {
    commands: &'a [CommandDefinition],
}
