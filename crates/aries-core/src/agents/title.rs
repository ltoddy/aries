use rig::client::CompletionClient;
use rig::completion::{self, Message};

use crate::agents::{AGENT_LOOP_MAX_TURNS, AriesAgent};

const PREAMBLE: &str = include_str!("prompts/title.txt");
const NAME: &str = "Namer";
const DESCRIPTION: &str = "用于生成对话标题的智能体。";

pub struct TitleAgent<M>
where
    M: completion::CompletionModel,
{
    inner: AriesAgent<M>,
}

impl<M> TitleAgent<M>
where
    M: completion::CompletionModel + 'static,
{
    pub fn new<C>(client: C, model: &str) -> Self
    where
        C: CompletionClient<CompletionModel = M>,
    {
        let agent = client
            .agent(model)
            .name(NAME)
            .description(DESCRIPTION)
            .preamble(PREAMBLE)
            .default_max_turns(AGENT_LOOP_MAX_TURNS)
            .build();

        Self { inner: AriesAgent::new(agent, NAME, PREAMBLE) }
    }

    pub async fn generate(&mut self, input: &str, history: &[Message]) -> anyhow::Result<String> {
        self.inner.completion(input, history).await
    }
}
