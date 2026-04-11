use std::future::Future;

use anyhow::Context;
use aries_config::AriesConfig;
use aries_context::GlobalContext;
use aries_core::compaction::CompactionAgent;
use aries_core::{AgentType, AgentWrapper};
use futures::{StreamExt, pin_mut};
use rig::agent::{FinalResponse, MultiTurnStreamItem, PromptHook, Text};
use rig::completion::{self, Message};
use rig::providers::{azure, openai};
use rig::streaming::StreamedAssistantContent;

pub struct SessionInner<M, P = ()>
where
    M: completion::CompletionModel,
    P: PromptHook<M>,
{
    id: String,
    agent: AgentWrapper<M, P>,
    compaction_agent: CompactionAgent<M>,
    history: Vec<Message>,
    base_history_len: usize,
}

impl<M, P> SessionInner<M, P>
where
    M: completion::CompletionModel,
    P: PromptHook<M>,
{
    fn new(
        id: String,
        context: &GlobalContext,
        agent: AgentWrapper<M, P>,
        compaction_agent: CompactionAgent<M>,
    ) -> Self {
        let history = vec![Message::user(format!("当前目录：{}", context.current_dir.display()))];
        let base_history_len = history.len();

        Self { id, agent, compaction_agent, history, base_history_len }
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn history(&self) -> &[Message] {
        &self.history
    }

    fn clear_history(&mut self) {
        self.history.truncate(self.base_history_len);
    }

    fn update_history_from_response(&mut self, response: &FinalResponse) {
        if let Some(history) = response.history() {
            self.history = history.to_vec();
        }
    }
}

impl<P> SessionInner<openai::CompletionModel, P>
where
    P: PromptHook<openai::CompletionModel> + Clone + 'static,
{
    async fn prompt<F, Fut>(&mut self, prompt: &str, mut on_text: F) -> anyhow::Result<()>
    where
        F: FnMut(String) -> Fut,
        Fut: Future<Output = anyhow::Result<()>>,
    {
        let _ = self.compact_if_needed().await?;
        let history = self.history.clone();
        let stream = self.agent.stream_prompt(prompt, &history).await;
        pin_mut!(stream);
        let mut final_res = FinalResponse::empty();

        while let Some(chunk) = stream.next().await {
            match chunk? {
                MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::Text(Text { text })) => {
                    on_text(text).await?;
                },
                MultiTurnStreamItem::FinalResponse(response) => final_res = response,
                _ => {},
            }
        }

        self.update_history_from_response(&final_res);
        Ok(())
    }

    async fn compact_if_needed(&mut self) -> anyhow::Result<Option<String>> {
        let Some(summary) = self.compaction_agent.compact(self.history.clone()).await? else {
            return Ok(None);
        };

        self.history.truncate(self.base_history_len);
        self.history.push(Message::assistant(summary.clone()));

        Ok(Some(summary))
    }
}

impl<P> SessionInner<azure::CompletionModel, P>
where
    P: PromptHook<azure::CompletionModel> + Clone + 'static,
{
    async fn prompt<F, Fut>(&mut self, prompt: &str, mut on_text: F) -> anyhow::Result<()>
    where
        F: FnMut(String) -> Fut,
        Fut: Future<Output = anyhow::Result<()>>,
    {
        let _ = self.compact_if_needed().await?;
        let history = self.history.clone();
        let stream = self.agent.stream_prompt(prompt, &history).await;
        pin_mut!(stream);
        let mut final_res = FinalResponse::empty();

        while let Some(chunk) = stream.next().await {
            match chunk? {
                MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::Text(Text { text })) => {
                    on_text(text).await?;
                },
                MultiTurnStreamItem::FinalResponse(response) => final_res = response,
                _ => {},
            }
        }

        self.update_history_from_response(&final_res);
        Ok(())
    }

    async fn compact_if_needed(&mut self) -> anyhow::Result<Option<String>> {
        let Some(summary) = self.compaction_agent.compact(self.history.clone()).await? else {
            return Ok(None);
        };

        self.history.truncate(self.base_history_len);
        self.history.push(Message::assistant(summary.clone()));

        Ok(Some(summary))
    }
}

pub enum Session<P = ()>
where
    P: PromptHook<openai::CompletionModel> + PromptHook<azure::CompletionModel>,
{
    OpenAICompatible(SessionInner<openai::CompletionModel, P>),
    Azure(SessionInner<azure::CompletionModel, P>),
}

impl Session<()> {
    pub fn new(id: String, context: &GlobalContext, config: AriesConfig) -> anyhow::Result<Self> {
        Self::new_with_task_hook(id, context, config, ())
    }
}

impl<P> Session<P>
where
    P: PromptHook<openai::CompletionModel> + PromptHook<azure::CompletionModel> + Clone + 'static,
{
    pub fn new_with_task_hook(
        id: String,
        context: &GlobalContext,
        config: AriesConfig,
        task_hook: P,
    ) -> anyhow::Result<Self> {
        match config.clone() {
            AriesConfig::OpenAICompatible(ref conf) => {
                let client = openai::CompletionsClient::builder()
                    .base_url(&conf.base_url)
                    .api_key(&conf.api_key)
                    .build()
                    .with_context(|| "Failed to create llm client")?;

                let agent = AgentWrapper::new(
                    client,
                    format!("Session Agent {}", id),
                    config.clone(),
                    AgentType::Build,
                    task_hook,
                );
                let compaction_agent = CompactionAgent::<openai::CompletionModel>::new(config)?;
                Ok(Self::OpenAICompatible(SessionInner::new(id, context, agent, compaction_agent)))
            },
            AriesConfig::Azure(ref conf) => {
                let client = azure::Client::builder()
                    .api_key(&conf.api_key)
                    .azure_endpoint(conf.azure_endpoint.to_owned())
                    .api_version(&conf.api_version)
                    .build()
                    .with_context(|| "Failed to create llm client")?;

                let agent = AgentWrapper::new(
                    client,
                    format!("Session Agent {}", id),
                    config.clone(),
                    AgentType::Build,
                    task_hook,
                );
                let compaction_agent = CompactionAgent::<azure::CompletionModel>::new(config)?;
                Ok(Self::Azure(SessionInner::new(id, context, agent, compaction_agent)))
            },
        }
    }

    pub async fn prompt<F, Fut>(&mut self, prompt: &str, on_text: F) -> anyhow::Result<()>
    where
        F: FnMut(String) -> Fut,
        Fut: Future<Output = anyhow::Result<()>>,
    {
        match self {
            Self::OpenAICompatible(session) => session.prompt(prompt, on_text).await,
            Self::Azure(session) => session.prompt(prompt, on_text).await,
        }
    }

    pub fn id(&self) -> &str {
        match self {
            Self::OpenAICompatible(session) => session.id(),
            Self::Azure(session) => session.id(),
        }
    }

    pub fn history(&self) -> &[Message] {
        match self {
            Self::OpenAICompatible(session) => session.history(),
            Self::Azure(session) => session.history(),
        }
    }

    pub fn clear_history(&mut self) {
        match self {
            Self::OpenAICompatible(session) => session.clear_history(),
            Self::Azure(session) => session.clear_history(),
        }
    }
}
