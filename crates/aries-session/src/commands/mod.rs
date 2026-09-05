mod builtin;
mod slash;

use aries_agent::AriesAgent;
use aries_compact::ContextCompactor;
use aries_event::Notifier;
use aries_extension::CommandDefinition;
use tokio_util::sync::CancellationToken;

pub use self::builtin::BUILTIN_COMMANDS;

pub struct CommandsExecutor<'a> {
    slash_commands_executor: slash::SlashCommandsExecutor<'a>,
    builtin_commands_executor: builtin::BuiltinCommandsExecutor<'a>,
}

impl<'a> CommandsExecutor<'a> {
    pub fn new(
        agent: &'a AriesAgent,
        commands: &'a [CommandDefinition],
        session_id: &'a str,
        compactor: ContextCompactor,
        notifier: Notifier,
    ) -> Self {
        let slash_commands_executor = slash::SlashCommandsExecutor::new(agent, commands);
        let builtin_commands_executor =
            builtin::BuiltinCommandsExecutor::new(agent, session_id, compactor, notifier);

        Self { slash_commands_executor, builtin_commands_executor }
    }

    pub async fn execute<F, Fut>(
        &mut self,
        input: impl AsRef<str>,
        receiver: &tokio::sync::Mutex<
            tokio::sync::mpsc::UnboundedReceiver<aries_event::AgentEvent>,
        >,
        cancel_token: &CancellationToken,
        callback: F,
    ) -> bool
    where
        F: Fn(aries_event::AgentEvent) -> Fut + Clone,
        Fut: Future<Output = ()>,
    {
        let input = input.as_ref();
        let input = input.trim();

        let (command, args) = if let Some((first, rest)) = input.split_once(' ') {
            (first, rest)
        } else {
            (input, "")
        };

        if self.builtin_commands_executor.is_builtin_command(command) {
            return self.builtin_commands_executor.execute(command, args).await;
        }
        self.slash_commands_executor.execute(command, args, receiver, cancel_token, callback).await
    }
}
