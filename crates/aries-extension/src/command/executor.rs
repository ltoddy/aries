use crate::command::CommandDefinition;

#[derive(Debug)]
pub struct SlashCommandsExecutor<'a> {
    commands: &'a [CommandDefinition],
}

impl<'a> SlashCommandsExecutor<'a> {
    pub fn new(commands: &'a [CommandDefinition]) -> Self {
        Self { commands }
    }

    pub async fn execute(&self, input: impl AsRef<str>) -> Option<String> {
        let input = input.as_ref();
        let input = input.trim();

        let (command, args) = if let Some((first, rest)) = input.split_once(' ') {
            (first, rest)
        } else {
            (input, "")
        };

        let command = self.commands.iter().find(|c| c.frontmatter.name == command)?;

        let prompt = command.expand_arguments(args);
        Some(prompt)
    }
}
