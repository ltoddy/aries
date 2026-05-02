use std::future::Future;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::PathBuf;

use anyhow::Context;
use aries_config::AriesConfig;
use aries_context::GlobalContext;
use aries_core::agent_type::AgentType;
use aries_core::compaction::CompactionAgent;
use aries_core::language_server::{LspServerInfo, SharedLspClient, warm_up};
use aries_core::task_spawner::{NotificationReceiver, TaskSpawner};
use aries_core::{AgentBuilder, AriesAgent};
use futures::{StreamExt, pin_mut};
use rig::agent::{FinalResponse, MultiTurnStreamItem, PromptHook};
use rig::completion::Message;
use rig::providers::{azure, openai};
use rig::streaming::StreamedAssistantContent;
use tokio::sync::mpsc::UnboundedSender;

pub struct Session<P = ()>
where
    P: PromptHook<openai::CompletionModel> + PromptHook<azure::CompletionModel>,
{
    id: String,
    provider_agents: ProviderAgents<P>,
    task_notifications: NotificationReceiver,
    _lsp_client: Option<SharedLspClient>,
    history: Vec<Message>,
    history_tx: UnboundedSender<Vec<Message>>,
    base_len: usize,
    transcript_dir: PathBuf,

    _dir: PathBuf, // session 的数据存放的目录
}

enum ProviderAgents<P = ()>
where
    P: PromptHook<openai::CompletionModel> + PromptHook<azure::CompletionModel>,
{
    OpenAICompatible {
        agent: AriesAgent<openai::CompletionModel, P>,
        compaction_agent: CompactionAgent<openai::CompletionModel>,
    },
    Azure {
        agent: AriesAgent<azure::CompletionModel, P>,
        compaction_agent: CompactionAgent<azure::CompletionModel>,
    },
}

impl<P> Session<P>
where
    P: PromptHook<openai::CompletionModel> + PromptHook<azure::CompletionModel> + Clone + 'static,
{
    pub async fn new(
        id: String,
        gctx: &GlobalContext,
        config: AriesConfig,
        task_hook: P,
    ) -> anyhow::Result<Self> {
        let mut history = Vec::new();
        let transcript_dir = gctx.config_dir.join("transcripts");
        let mut lsp_client: Option<SharedLspClient> = None;

        if let Some(info) = LspServerInfo::detect(&gctx.current_dir)
            && info.installed()
            && let Ok(lsp) = warm_up(info, &gctx.current_dir).await
        {
            lsp_client = Some(lsp);
            history.push(Message::user("已检测到本项目适用的语言服务器，现已启动并开始预热。进行代码定义跳转、引用查找、符号查询或调用层级等语义级检索时，请优先使用 `lsp` 工具以获得更准确的结果。若首次调用时 `lsp` 尚未就绪（语言服务器可能仍在索引工作区），可稍后重试，或临时改用 `codesearch`、`grep`。"));
        }

        let dir = gctx.config_dir.join(format!("session-{:x}", {
            let mut hasher = DefaultHasher::new();
            gctx.current_dir.hash(&mut hasher);
            hasher.finish()
        }));

        if !dir.exists()
            && let Err(err) = tokio::fs::create_dir_all(&dir).await
        {
            eprintln!("Failed to create session directory: {err}");
        };

        let history_file_path = dir.join("chat-history.jsonl");
        if let Ok(prior) = crate::history::load_history(&history_file_path).await {
            history.extend_from_slice(&prior);
        }

        let base_len = history.len();

        let (provider_agents, task_notifications) = match config.clone() {
            AriesConfig::OpenAICompatible(ref conf) => {
                let client = openai::CompletionsClient::builder()
                    .base_url(&conf.base_url)
                    .api_key(&conf.api_key)
                    .build()
                    .with_context(|| "Failed to create llm client")?;

                let (spawner, task_notifications) = TaskSpawner::new();
                let agent = AgentBuilder::new(
                    client,
                    config.clone(),
                    AgentType::Build,
                    task_hook,
                    gctx.clone(),
                )
                .with_tools(spawner, lsp_client.clone())
                .await;
                let compaction_agent = CompactionAgent::<openai::CompletionModel>::new(config)?;
                (ProviderAgents::OpenAICompatible { agent, compaction_agent }, task_notifications)
            },
            AriesConfig::Azure(ref conf) => {
                let client = azure::Client::builder()
                    .api_key(&conf.api_key)
                    .azure_endpoint(conf.azure_endpoint.to_owned())
                    .api_version(&conf.api_version)
                    .build()
                    .with_context(|| "Failed to create llm client")?;

                let (spawner, task_notifications) = TaskSpawner::new();
                let agent = AgentBuilder::new(
                    client,
                    config.clone(),
                    AgentType::Build,
                    task_hook,
                    gctx.clone(),
                )
                .with_tools(spawner, lsp_client.clone())
                .await;
                let compaction_agent = CompactionAgent::<azure::CompletionModel>::new(config)?;
                (ProviderAgents::Azure { agent, compaction_agent }, task_notifications)
            },
        };

        let (history_tx, history_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<Message>>();
        tokio::spawn(crate::history::refresh_history(history_rx, history_file_path));

        Ok(Self {
            id,
            provider_agents,
            task_notifications,
            _lsp_client: lsp_client,
            history,
            history_tx,
            base_len,
            transcript_dir,
            _dir: dir,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn history(&self) -> &[Message] {
        &self.history
    }

    pub fn clear_history(&mut self) {
        self.history.truncate(self.base_len);
        let _ = self.history_tx.send(vec![]);
    }

    pub async fn prompt<F, Fut>(&mut self, prompt: &str, mut cb: Option<F>) -> anyhow::Result<()>
    where
        F: FnMut(MultiTurnStreamItem<()>) -> Fut,
        Fut: Future<Output = anyhow::Result<()>>,
    {
        self.drain_task_notifications();

        let stream = match &mut self.provider_agents {
            ProviderAgents::OpenAICompatible { agent, compaction_agent } => {
                CompactionAgent::<openai::CompletionModel>::micro_compact(
                    &mut self.history[self.base_len..],
                );
                if let Some(compressed) = compaction_agent
                    .auto_compact(&self.history[self.base_len..], &self.transcript_dir)
                    .await?
                {
                    self.history.truncate(self.base_len);
                    self.history.extend(compressed);
                }
                let snapshot = self.history.clone();
                agent.stream_prompt(prompt, &snapshot).await
            },
            ProviderAgents::Azure { agent, compaction_agent } => {
                CompactionAgent::<azure::CompletionModel>::micro_compact(&mut self.history);
                if let Some(compressed) = compaction_agent
                    .auto_compact(&self.history[self.base_len..], &self.transcript_dir)
                    .await?
                {
                    self.history.truncate(self.base_len);
                    self.history.extend(compressed);
                }
                let snapshot = self.history.clone();
                agent.stream_prompt(prompt, &snapshot).await
            },
        };

        pin_mut!(stream);
        let mut final_res = FinalResponse::empty();

        while let Some(Ok(chunk)) = stream.next().await {
            if let MultiTurnStreamItem::FinalResponse(response) = &chunk {
                final_res = response.clone();
            }
            if let Some(ref mut cb) = cb
                && let Some(stripped) = erase_provider_type(chunk)
            {
                cb(stripped).await?;
            }
        }

        if let Some(his) = final_res.history() {
            self.history = his.to_vec();
            let _ = self.history_tx.send(his.get(self.base_len..).unwrap_or(&[]).to_vec());
        }

        Ok(())
    }

    pub async fn compact(&mut self) -> anyhow::Result<()> {
        let compressed = match &mut self.provider_agents {
            ProviderAgents::OpenAICompatible { compaction_agent, .. } => {
                compaction_agent
                    .force_compact(&self.history[self.base_len..], &self.transcript_dir)
                    .await?
            },
            ProviderAgents::Azure { compaction_agent, .. } => {
                compaction_agent
                    .force_compact(&self.history[self.base_len..], &self.transcript_dir)
                    .await?
            },
        };
        if let Some(compressed) = compressed {
            self.history.truncate(self.base_len);
            self.history.extend(compressed);
        }
        Ok(())
    }

    fn drain_task_notifications(&mut self) {
        let notifications = self.task_notifications.drain();
        if notifications.is_empty() {
            return;
        }

        let notif_text: String = notifications
            .iter()
            .map(|n| {
                let mut parts =
                    format!("[task:{}] command={} exit_code={}", n.task_id, n.command, n.exit_code);
                if !n.stdout.is_empty() {
                    parts.push_str(&format!("\nstdout: {}", n.stdout));
                }
                if !n.stderr.is_empty() {
                    parts.push_str(&format!("\nstderr: {}", n.stderr));
                }
                parts
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        self.history
            .push(Message::user(format!("<task-results>\n{}\n</task-results>", notif_text)));
        self.history.push(Message::assistant("Noted task results."));
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
