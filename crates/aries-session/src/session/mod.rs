mod history;
mod hook;

use std::future::Future;
use std::path::{Path, PathBuf};

use anyhow::Context;
use aries_config::AriesConfig;
use aries_core::agents::{AgentBuilder, AgentType, AriesAgent, CompactionAgent};
use aries_core::compact;
use aries_core::language_server::{LspServerInfo, SharedLspClient, warm_up};
use futures::{StreamExt, pin_mut};
use rig_core::agent::{FinalResponse, MultiTurnStreamItem};
use rig_core::completion::Message;
use rig_core::providers::{azure, deepseek, openai};
use rig_core::streaming::StreamedAssistantContent;
use tokio::fs::create_dir_all;
use tokio_util::sync::CancellationToken;

use crate::session::history::ChatHistory;

#[derive(Clone)]
pub struct Session {
    id: String,
    config: AriesConfig,
    cwd: PathBuf,
    agents: ProviderAgents,
    #[allow(unused)]
    lsp_client: Option<SharedLspClient>,
    chat_history: ChatHistory,
    root_dir: PathBuf,
    cancel_token: CancellationToken,
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
    DeepSeek {
        agent: AriesAgent<deepseek::CompletionModel>,
        compaction_agent: CompactionAgent<deepseek::CompletionModel>,
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
        let cwd = cwd.as_ref();
        let root_dir = root_dir.as_ref();
        if !root_dir.exists() {
            create_dir_all(&root_dir).await.with_context(|| {
                format!("Failed to created session directory: {}", root_dir.display())
            })?;
        }

        let transcript_dir = root_dir.join("transcripts");

        let lsp_client = Self::warm_up_lsp(cwd).await;
        let agents = Self::create_agents(
            AgentType::Build,
            config.clone(),
            cwd,
            transcript_dir,
            lsp_client.clone(),
        )
        .await?;
        let chat_history = ChatHistory::new(root_dir.join(Self::FILENAME)).await;
        let cancel_token = CancellationToken::new();

        Ok(Self {
            id,
            config,
            cwd: cwd.to_path_buf(),
            agents,
            lsp_client,
            chat_history,
            root_dir: root_dir.to_path_buf(),
            cancel_token,
        })
    }

    pub async fn load(
        id: String,
        config: AriesConfig,
        root_dir: impl AsRef<Path>,
        cwd: impl AsRef<Path>,
    ) -> anyhow::Result<Self> {
        let cwd = cwd.as_ref();
        let root_dir = root_dir.as_ref();
        let transcript_dir = root_dir.join("transcripts");

        let lsp_client = Self::warm_up_lsp(cwd).await;
        let agents = Self::create_agents(
            AgentType::Build,
            config.clone(),
            cwd,
            transcript_dir,
            lsp_client.clone(),
        )
        .await?;
        let chat_history = ChatHistory::new(root_dir.join(Self::FILENAME)).await;
        let cancel_token = CancellationToken::new();

        Ok(Self {
            id,
            config,
            cwd: cwd.to_path_buf(),
            agents,
            lsp_client,
            chat_history,
            root_dir: root_dir.to_path_buf(),
            cancel_token,
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
            ProviderAgents::DeepSeek { agent, .. } => agent.system_prompt(),
        }
    }

    pub fn history(&self) -> &[Message] {
        self.chat_history.history()
    }

    pub fn clear_history(&mut self) {
        self.chat_history.clear();
    }

    pub fn cancel(&self) {
        self.cancel_token.cancel();
    }

    pub async fn switch_agent(&mut self, agent_type: AgentType) -> anyhow::Result<()> {
        let transcript_dir = self.root_dir.join("transcripts");
        let agents = Self::create_agents(
            agent_type,
            self.config.clone(),
            self.cwd.clone(),
            transcript_dir,
            self.lsp_client.clone(),
        )
        .await?;
        self.agents = agents;
        Ok(())
    }

    pub fn dir(&self) -> PathBuf {
        self.root_dir.clone()
    }

    pub async fn prompt<F, Fut>(&mut self, prompt: &str, mut cb: Option<F>) -> anyhow::Result<()>
    where
        F: FnMut(MultiTurnStreamItem<()>) -> Fut,
        Fut: Future<Output = anyhow::Result<()>>,
    {
        self.cancel_token = CancellationToken::new();
        let cancel_token = self.cancel_token.clone();

        let final_res = match &mut self.agents {
            ProviderAgents::OpenAICompatible { agent, compaction_agent } => {
                compact::micro_compact(self.chat_history.history_mut());

                let snapshot = self.chat_history.history().to_vec();
                let stream = agent.stream_prompt(prompt, &snapshot).await;
                let final_res = Self::consume_stream(stream, &mut cb, cancel_token).await?;

                if final_res.usage().total_tokens > compact::TOKEN_THRESHOLD {
                    println!("\n🔄 触发上下文压缩...");
                    if let Some(compressed) =
                        compaction_agent.compact(self.chat_history.history()).await
                    {
                        self.chat_history.reset(&compressed);
                    }
                }

                final_res
            },
            ProviderAgents::Azure { agent, compaction_agent } => {
                compact::micro_compact(self.chat_history.history_mut());

                let snapshot = self.chat_history.history().to_vec();
                let stream = agent.stream_prompt(prompt, &snapshot).await;

                let final_res = Self::consume_stream(stream, &mut cb, cancel_token).await?;
                if final_res.usage().total_tokens > compact::TOKEN_THRESHOLD {
                    println!("\n🔄 触发上下文压缩...");
                    if let Some(compressed) =
                        compaction_agent.compact(self.chat_history.history()).await
                    {
                        self.chat_history.reset(&compressed);
                    }
                }

                final_res
            },
            ProviderAgents::DeepSeek { agent, compaction_agent } => {
                compact::micro_compact(self.chat_history.history_mut());

                let snapshot = self.chat_history.history().to_vec();
                let stream = agent.stream_prompt(prompt, &snapshot).await;

                let final_res = Self::consume_stream(stream, &mut cb, cancel_token).await?;
                if final_res.usage().total_tokens > compact::TOKEN_THRESHOLD {
                    println!(
                        "\n🔄 total token: {} 触发上下文压缩...",
                        final_res.usage().total_tokens
                    );
                    if let Some(compressed) =
                        compaction_agent.compact(self.chat_history.history()).await
                    {
                        self.chat_history.reset(&compressed);
                    }
                }

                final_res
            },
        };

        if let Some(his) = final_res.history() {
            self.chat_history.extend(his.iter().cloned());
            self.chat_history.persist();
        }

        Ok(())
    }

    pub async fn compact(&mut self) -> bool {
        let compressed = match &mut self.agents {
            ProviderAgents::OpenAICompatible { compaction_agent, .. } => {
                compaction_agent.compact(self.chat_history.history()).await
            },
            ProviderAgents::Azure { compaction_agent, .. } => {
                compaction_agent.compact(self.chat_history.history()).await
            },
            ProviderAgents::DeepSeek { compaction_agent, .. } => {
                compaction_agent.compact(self.chat_history.history()).await
            },
        };
        if let Some(compressed) = compressed {
            self.chat_history.reset(&compressed);
            return true;
        }
        false
    }

    async fn consume_stream<F, Fut, R>(
        stream: rig_core::agent::StreamingResult<R>,
        cb: &mut Option<F>,
        cancel_token: CancellationToken,
    ) -> anyhow::Result<FinalResponse>
    where
        F: FnMut(MultiTurnStreamItem<()>) -> Fut,
        Fut: Future<Output = anyhow::Result<()>>,
    {
        pin_mut!(stream);
        let mut final_res = FinalResponse::empty();

        loop {
            tokio::select! {
                biased;
                _ = cancel_token.cancelled() => {
                    break;
                }
                item = stream.next() => {
                    match item {
                        Some(Ok(chunk)) => {
                            if let MultiTurnStreamItem::FinalResponse(response) = &chunk {
                                final_res = response.clone();
                            }
                            if let Some(cb) = cb
                                && let Some(stripped) = erase_provider_type(chunk)
                            {
                                cb(stripped).await?;
                            }
                        }
                        Some(Err(_)) | None => break,
                    }
                }
            }
        }

        Ok(final_res)
    }

    async fn create_agents(
        agent_type: AgentType,
        config: AriesConfig,
        cwd: impl AsRef<Path>,
        transcript_dir: impl AsRef<Path>,
        lsp_client: Option<SharedLspClient>,
    ) -> anyhow::Result<ProviderAgents> {
        let cwd = cwd.as_ref().to_path_buf();

        let agents = match config.clone() {
            AriesConfig::OpenAICompatible(ref conf) => {
                let model = config.model().to_owned();
                let client = openai::CompletionsClient::builder()
                    .base_url(&conf.base_url)
                    .api_key(&conf.api_key)
                    .build()
                    .with_context(|| "Failed to create llm client")?;

                let agent = AgentBuilder::<openai::CompletionsClient>::new(
                    client.clone(),
                    config,
                    agent_type,
                    cwd,
                )
                .with_tools(lsp_client)
                .await;
                let compaction_agent = CompactionAgent::new(client, model, transcript_dir);
                ProviderAgents::OpenAICompatible { agent, compaction_agent }
            },
            AriesConfig::Azure(ref conf) => {
                let model = config.model().to_owned();
                let client = azure::Client::builder()
                    .api_key(&conf.api_key)
                    .azure_endpoint(conf.azure_endpoint.to_owned())
                    .api_version(&conf.api_version)
                    .build()
                    .with_context(|| "Failed to create llm client")?;

                let agent =
                    AgentBuilder::<azure::Client>::new(client.clone(), config, agent_type, cwd)
                        .with_tools(lsp_client)
                        .await;
                let compaction_agent = CompactionAgent::new(client, model, transcript_dir);
                ProviderAgents::Azure { agent, compaction_agent }
            },
            AriesConfig::DeepSeek(ref conf) => {
                let model = config.model().to_owned();
                let client = deepseek::Client::builder()
                    .api_key(&conf.api_key)
                    .build()
                    .with_context(|| "Failed to create llm client")?;

                let agent =
                    AgentBuilder::<deepseek::Client>::new(client.clone(), config, agent_type, cwd)
                        .with_tools(lsp_client)
                        .await;
                let compaction_agent = CompactionAgent::new(client, model, transcript_dir);
                ProviderAgents::DeepSeek { agent, compaction_agent }
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
