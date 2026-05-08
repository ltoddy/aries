use std::future::Future;
use std::path::{Path, PathBuf};

use anyhow::Context;
use aries_config::AriesConfig;
use aries_core::agents::{AgentBuilder, AgentType, AriesAgent, CompactionAgent};
use aries_core::language_server::{LspServerInfo, SharedLspClient, warm_up};
use futures::{StreamExt, pin_mut};
use rig::agent::{FinalResponse, MultiTurnStreamItem, PromptHook};
use rig::completion::Message;
use rig::providers::{azure, deepseek, openai};
use rig::streaming::StreamedAssistantContent;
use tokio::fs::create_dir_all;

use crate::session::history::ChatHistory;

mod history;

#[derive(Clone)]
pub struct Session {
    id: String,
    agents: ProviderAgents,
    #[allow(unused)]
    lsp_client: Option<SharedLspClient>,
    chat_history: ChatHistory,
    transcript_dir: PathBuf,
    root_dir: PathBuf, // session 的数据存放的目录
}

#[derive(Clone)]
enum ProviderAgents {
    OpenAICompatible {
        agent: AriesAgent<openai::CompletionModel>,
        compaction_agent: CompactionAgent<openai::CompletionModel>,
    },
    Azure {
        agent: AriesAgent<azure::CompletionModel>,
        compaction_agent: CompactionAgent<azure::CompletionModel>,
    },
}

impl Session {
    const FILENAME: &str = "chat-history.jsonl";

    pub async fn new(
        id: String,
        config: AriesConfig,
        root_dir: impl AsRef<Path>,
        cwd: impl AsRef<Path>,
    ) -> anyhow::Result<Self> {
        let root_dir = root_dir.as_ref();
        if !root_dir.exists() {
            create_dir_all(&root_dir).await.with_context(|| {
                format!("Failed to created session directory: {}", root_dir.display())
            })?;
        }

        let lsp_client = Self::warm_up_lsp(&cwd).await;
        let agents = Self::create_agents(config, &cwd, lsp_client.clone()).await?;
        let chat_history = ChatHistory::new(root_dir.join(Self::FILENAME)).await;

        Ok(Self {
            id,
            agents,
            lsp_client,
            chat_history,
            transcript_dir: root_dir.join("transcripts"),
            root_dir: root_dir.to_path_buf(),
        })
    }

    pub async fn load(
        id: String,
        config: AriesConfig,
        root_dir: impl AsRef<Path>,
        cwd: impl AsRef<Path>,
    ) -> anyhow::Result<Self> {
        let root_dir = root_dir.as_ref();

        let lsp_client = Self::warm_up_lsp(&cwd).await;
        let agents = Self::create_agents(config, &cwd, lsp_client.clone()).await?;
        let chat_history = ChatHistory::new(root_dir.join(Self::FILENAME)).await;

        Ok(Self {
            id,
            agents,
            lsp_client,
            chat_history,
            transcript_dir: root_dir.join("transcripts"),
            root_dir: root_dir.to_path_buf(),
        })
    }
}

impl Session {
    pub const PREFIX: &str = "session-";

    pub fn id(&self) -> String {
        self.id.clone()
    }

    pub fn system_prompt(&self) -> &str {
        match &self.agents {
            ProviderAgents::OpenAICompatible { agent, .. } => agent.system_prompt(),
            ProviderAgents::Azure { agent, .. } => agent.system_prompt(),
        }
    }

    pub fn history(&self) -> &[Message] {
        self.chat_history.history()
    }

    pub fn clear_history(&mut self) {
        self.chat_history.clear();
    }

    pub fn dir(&self) -> PathBuf {
        self.root_dir.clone()
    }

    pub async fn prompt<F, Fut, P>(
        &mut self,
        prompt: &str,
        mut cb: Option<F>,
        hook: P,
    ) -> anyhow::Result<()>
    where
        F: FnMut(MultiTurnStreamItem<()>) -> Fut,
        Fut: Future<Output = anyhow::Result<()>>,
        P: PromptHook<deepseek::CompletionModel>
            + PromptHook<openai::CompletionModel>
            + PromptHook<azure::CompletionModel>
            + 'static,
    {
        let final_res = match &mut self.agents {
            ProviderAgents::OpenAICompatible { agent, compaction_agent } => {
                CompactionAgent::<openai::CompletionModel>::micro_compact(
                    self.chat_history.history_mut(),
                );
                if let Some(compressed) = compaction_agent
                    .auto_compact(self.chat_history.history(), &self.transcript_dir)
                    .await?
                {
                    self.chat_history.extend(compressed);
                }
                let snapshot = self.chat_history.history().to_vec();
                let stream = agent.stream_prompt(prompt, &snapshot, hook).await;
                Self::consume_stream(stream, &mut cb).await?
            },
            ProviderAgents::Azure { agent, compaction_agent } => {
                CompactionAgent::<azure::CompletionModel>::micro_compact(
                    self.chat_history.history_mut(),
                );
                if let Some(compressed) = compaction_agent
                    .auto_compact(self.chat_history.history(), &self.transcript_dir)
                    .await?
                {
                    self.chat_history.extend(compressed);
                }
                let snapshot = self.chat_history.history().to_vec();
                let stream = agent.stream_prompt(prompt, &snapshot, hook).await;
                Self::consume_stream(stream, &mut cb).await?
            },
        };

        if let Some(his) = final_res.history() {
            self.chat_history.extend(his.iter().cloned());
            self.chat_history.persist();
        }

        Ok(())
    }

    pub async fn compact(&mut self) -> anyhow::Result<()> {
        let compressed = match &mut self.agents {
            ProviderAgents::OpenAICompatible { compaction_agent, .. } => {
                compaction_agent
                    .force_compact(self.chat_history.history(), &self.transcript_dir)
                    .await?
            },
            ProviderAgents::Azure { compaction_agent, .. } => {
                compaction_agent
                    .force_compact(self.chat_history.history(), &self.transcript_dir)
                    .await?
            },
        };
        if let Some(compressed) = compressed {
            self.chat_history.extend(compressed);
        }
        Ok(())
    }

    async fn consume_stream<F, Fut, R>(
        stream: rig::agent::StreamingResult<R>,
        cb: &mut Option<F>,
    ) -> anyhow::Result<FinalResponse>
    where
        F: FnMut(MultiTurnStreamItem<()>) -> Fut,
        Fut: Future<Output = anyhow::Result<()>>,
    {
        pin_mut!(stream);
        let mut final_res = FinalResponse::empty();

        while let Some(Ok(chunk)) = stream.next().await {
            if let MultiTurnStreamItem::FinalResponse(response) = &chunk {
                final_res = response.clone();
            }
            if let Some(cb) = cb
                && let Some(stripped) = erase_provider_type(chunk)
            {
                cb(stripped).await?;
            }
        }

        Ok(final_res)
    }

    async fn create_agents(
        config: AriesConfig,
        cwd: impl AsRef<Path>,
        lsp_client: Option<SharedLspClient>,
    ) -> anyhow::Result<ProviderAgents> {
        let cwd = cwd.as_ref();

        let agents = match config.clone() {
            AriesConfig::OpenAICompatible(ref conf) => {
                let client = openai::CompletionsClient::builder()
                    .base_url(&conf.base_url)
                    .api_key(&conf.api_key)
                    .build()
                    .with_context(|| "Failed to create llm client")?;

                let agent = AgentBuilder::new(
                    client.clone(),
                    config.clone(),
                    AgentType::Build,
                    cwd.to_path_buf(),
                )
                .with_tools(lsp_client)
                .await;
                let compaction_agent = CompactionAgent::new(client, config.model());
                ProviderAgents::OpenAICompatible { agent, compaction_agent }
            },
            AriesConfig::Azure(ref conf) => {
                let client = azure::Client::builder()
                    .api_key(&conf.api_key)
                    .azure_endpoint(conf.azure_endpoint.to_owned())
                    .api_version(&conf.api_version)
                    .build()
                    .with_context(|| "Failed to create llm client")?;

                let agent = AgentBuilder::new(
                    client.clone(),
                    config.clone(),
                    AgentType::Build,
                    cwd.to_path_buf(),
                )
                .with_tools(lsp_client)
                .await;
                let compaction_agent = CompactionAgent::new(client, config.model());
                ProviderAgents::Azure { agent, compaction_agent }
            },
        };

        Ok(agents)
    }

    async fn warm_up_lsp(dir: impl AsRef<Path>) -> Option<SharedLspClient> {
        let dir = dir.as_ref();

        if let Some(info) = LspServerInfo::detect(dir)
            && info.installed()
            && let Ok(lsp) = warm_up(info, dir).await
        {
            return Some(lsp);
        }

        None
    }
}

/// 把 `MultiTurnStreamItem<R>` 中 provider 相关的泛型 `R` 擦除为 `()`。
///
/// `StreamedAssistantContent::Final(R)` 是 provider 内部的原始流结束负载，
/// 上层不需要它（真正的最终结果是 `MultiTurnStreamItem::FinalResponse`）。
/// 遇到 `Final(R)` 时返回 `None`，其余变体安全映射为
/// `MultiTurnStreamItem<()>`。
fn erase_provider_type<R>(item: MultiTurnStreamItem<R>) -> Option<MultiTurnStreamItem<()>> {
    match item {
        MultiTurnStreamItem::StreamAssistantItem(assistant) => match assistant {
            StreamedAssistantContent::Final(_) => None,
            StreamedAssistantContent::Text(t) => {
                Some(MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::Text(t)))
            },
            StreamedAssistantContent::ToolCall { tool_call, internal_call_id } => {
                Some(MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::ToolCall {
                    tool_call,
                    internal_call_id,
                }))
            },
            StreamedAssistantContent::ToolCallDelta { id, internal_call_id, content } => {
                Some(MultiTurnStreamItem::StreamAssistantItem(
                    StreamedAssistantContent::ToolCallDelta { id, internal_call_id, content },
                ))
            },
            StreamedAssistantContent::Reasoning(r) => Some(
                MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::Reasoning(r)),
            ),
            StreamedAssistantContent::ReasoningDelta { id, reasoning } => {
                Some(MultiTurnStreamItem::StreamAssistantItem(
                    StreamedAssistantContent::ReasoningDelta { id, reasoning },
                ))
            },
        },
        MultiTurnStreamItem::StreamUserItem(user) => {
            Some(MultiTurnStreamItem::StreamUserItem(user))
        },
        MultiTurnStreamItem::FinalResponse(resp) => Some(MultiTurnStreamItem::FinalResponse(resp)),
        _ => None,
    }
}
