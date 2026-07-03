use rig_core::client::CompletionClient;
use rig_core::completion::{self, Message};

use crate::AriesResult;
use crate::agents::{AGENT_LOOP_MAX_TURNS, AriesAgent};

const PREAMBLE: &str = include_str!("prompts/title.md");
const NAME: &str = "Namer";
const DESCRIPTION: &str = "用于生成对话标题的智能体。";

pub struct TitleAgent<M>
where
    M: completion::CompletionModel + 'static,
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

        Self { inner: AriesAgent::new(agent, NAME, PREAMBLE, None) }
    }

    pub async fn generate(&mut self, input: &str, history: &[Message]) -> AriesResult<String> {
        let final_res = self.inner.prompt(input, history, ()).await?;

        Ok(final_res.response().to_owned())
    }
}
