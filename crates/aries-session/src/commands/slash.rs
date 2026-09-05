use std::future::Future;

use aries_agent::AriesAgent;
use aries_event::AgentEvent;
use aries_extension::CommandDefinition;
use rig::message::Message;
use tokio::sync::Mutex;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio_util::sync::CancellationToken;

pub struct SlashCommandsExecutor<'a> {
    agent: &'a AriesAgent,
    commands: &'a [CommandDefinition],
}

impl<'a> SlashCommandsExecutor<'a> {
    pub fn new(agent: &'a AriesAgent, commands: &'a [CommandDefinition]) -> Self {
        Self { agent, commands }
    }

    pub async fn execute<F, Fut>(
        &self,
        command: impl AsRef<str>,
        args: impl AsRef<str>,
        receiver: &Mutex<UnboundedReceiver<AgentEvent>>,
        cancel_token: &CancellationToken,
        callback: F,
    ) -> bool
    where
        F: Fn(AgentEvent) -> Fut + Clone,
        Fut: Future<Output = ()>,
    {
        let command = command.as_ref();
        let args = args.as_ref();

        let Some(command) = self.commands.iter().find(|c| c.frontmatter.name == command) else {
            return false;
        };

        let prompt = command.expand_arguments(args);
        let future = self.agent.prompt::<_, Message, _>(prompt, [], ());
        tokio::pin!(future);

        loop {
            let event = {
                let mut receiver = receiver.lock().await;
                tokio::select! {
                    biased;
                    _ = cancel_token.cancelled() => return true,
                    event = receiver.recv() => event,
                    res = &mut future => return res.is_ok(),
                }
            };

            match event {
                Some(event) => callback(event).await,
                None => return true,
            }
        }
    }
}
