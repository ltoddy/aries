use std::ops::{Deref, DerefMut};

use aries_config::AriesConfig;
use aries_context::GlobalContext;
use aries_theme::Theme;
use futures::StreamExt;
use rig::agent::{MultiTurnStreamItem, Text};
use rig::completion::Message;
use rig::streaming::StreamedAssistantContent;

use crate::compaction::CompactionAgent;
use crate::{AgentType, AgentWrapper};

pub struct OrchestrateAgent {
    inner: AgentWrapper,
    pub compaction_agent: CompactionAgent,
}

impl OrchestrateAgent {
    pub fn new(context: GlobalContext, config: AriesConfig) -> anyhow::Result<Self> {
        let name = String::from("Aries");
        let history = vec![Message::user(format!("当前目录：{}", context.current_dir.display()))];

        let inner = AgentWrapper::new(name, config.clone(), AgentType::Orchestrate, history)?;
        let compaction_agent = CompactionAgent::new(config.clone())?;

        Ok(Self { inner, compaction_agent })
    }

    pub fn stream_prompt_v2<'a>(
        &mut self,
        input: &str,
    ) -> impl futures::Stream<Item = anyhow::Result<String>> {
        async_stream::try_stream! {
            let theme = Theme::default();

            {
                let stream = self.inner.stream_prompt(input).await;
                tokio::pin!(stream);

                while let Some(chunk) = stream.next().await {
                    match chunk {
                        Ok(MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::Text(Text { text }))) => {
                            yield text;
                        },
                        Ok(MultiTurnStreamItem::FinalResponse(_res)) => { },
                        Err(e) => eprintln!("\n{}: {}", theme.red_text("Error streaming_chunk"), e),
                        Ok(_) => {},
                    }
                }
            }

            let messages = self.inner.history.clone();
            if let Ok(Some(summary)) = self.compaction_agent.compact(messages).await {
                self.clear_history(1);
                self.inner.history.push(Message::assistant(summary));
            }
        }
    }
}

impl Deref for OrchestrateAgent {
    type Target = AgentWrapper;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for OrchestrateAgent {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
