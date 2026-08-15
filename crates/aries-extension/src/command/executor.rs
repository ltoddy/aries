use crate::command::CommandDefinition;

pub struct SlashCommandsExecutor<'a> {
    commands: &'a [CommandDefinition],
}
