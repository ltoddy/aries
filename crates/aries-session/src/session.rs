use std::future::Future;
use std::os::unix::prelude::OsStrExt;
use std::path::PathBuf;

use anyhow::Context;
use aries_config::AriesConfig;
use aries_context::GlobalContext;
use aries_core::compaction::CompactionAgent;
use aries_core::language_server::{LspServerInfo, SharedLspClient, warm_up};
use aries_core::task_spawner::{NotificationReceiver, TaskSpawner};
use aries_core::{AgentType, AgentWrapper};
use futures::{StreamExt, pin_mut};
use rig::agent::{FinalResponse, MultiTurnStreamItem, PromptHook, Text};
use rig::completion::Message;
use rig::message::{ReasoningContent, ToolResultContent};
use rig::providers::{azure, openai};
use rig::streaming::{StreamedAssistantContent, StreamedUserContent};

use crate::event::StreamEvent;

pub struct Session<P = ()>
where
    P: PromptHook<openai::CompletionModel> + PromptHook<azure::CompletionModel>,
{
    id: String,
    provider_agents: ProviderAgents<P>,
    task_notifications: NotificationReceiver,
    _lsp_client: Option<SharedLspClient>,
    history: Vec<Message>,
    base_len: usize,
    transcript_dir: PathBuf,

    dir: PathBuf, // session 的数据存放的目录
}

enum ProviderAgents<P = ()>
where
    P: PromptHook<openai::CompletionModel> + PromptHook<azure::CompletionModel>,
{
    OpenAICompatible {
        agent: AgentWrapper<openai::CompletionModel, P>,
        compaction_agent: CompactionAgent<openai::CompletionModel>,
    },
    Azure {
        agent: AgentWrapper<azure::CompletionModel, P>,
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
        let mut history = vec![Message::user(format!("当前目录：{}", gctx.current_dir.display(),))];
        let transcript_dir = gctx.config_dir.join("transcripts");
        let mut lsp_client: Option<SharedLspClient> = None;

        if let Some(info) = LspServerInfo::detect(&gctx.current_dir)
            && info.installed()
        {
            if let Ok(lsp) = warm_up(info, &gctx.current_dir).await {
                lsp_client = Some(lsp);
                history.push(Message::user("已检测到本项目适用的语言服务器，现已启动并开始预热。进行代码定义跳转、引用查找、符号查询或调用层级等语义级检索时，请优先使用 `lsp` 工具以获得更准确的结果。若首次调用时 `lsp` 尚未就绪（语言服务器可能仍在索引工作区），可稍后重试，或临时改用 `codesearch`、`grep`。"));
            }
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
                let agent = AgentWrapper::new(client, config.clone(), AgentType::Build, task_hook)
                    .with_tools(spawner, lsp_client.clone());
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
                let agent = AgentWrapper::new(client, config.clone(), AgentType::Build, task_hook)
                    .with_tools(spawner, lsp_client.clone());
                let compaction_agent = CompactionAgent::<azure::CompletionModel>::new(config)?;
                (ProviderAgents::Azure { agent, compaction_agent }, task_notifications)
            },
        };

        let dir = gctx
            .config_dir
            .join(format!("session-{}", blake3::hash(gctx.current_dir.as_os_str().as_bytes())));

        if !dir.exists() {
            if let Err(err) = tokio::fs::create_dir_all(&dir).await {
                eprintln!("Failed to create session directory: {err}");
            };
        }

        Ok(Self {
            id,
            provider_agents,
            task_notifications,
            _lsp_client: lsp_client,
            history,
            base_len,
            transcript_dir,
            dir,
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
    }

    pub async fn prompt<F, Fut>(&mut self, prompt: &str, mut cb: Option<F>) -> anyhow::Result<()>
    where
        F: FnMut(StreamEvent) -> Fut,
        Fut: Future<Output = anyhow::Result<()>>,
    {
        self.drain_task_notifications();

        let stream = match &mut self.provider_agents {
            ProviderAgents::OpenAICompatible { agent, compaction_agent } => {
                CompactionAgent::<openai::CompletionModel>::micro_compact(&mut self.history);
                if let Some(compressed) =
                    compaction_agent.auto_compact(&self.history, &self.transcript_dir).await?
                {
                    self.history.truncate(self.base_len);
                    self.history.extend(compressed);
                }
                let snapshot = self.history.clone();
                agent.stream_prompt(prompt, &snapshot).await
            },
            ProviderAgents::Azure { agent, compaction_agent } => {
                CompactionAgent::<azure::CompletionModel>::micro_compact(&mut self.history);
                if let Some(compressed) =
                    compaction_agent.auto_compact(&self.history, &self.transcript_dir).await?
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

        while let Some(chunk) = stream.next().await {
            match chunk? {
                MultiTurnStreamItem::StreamAssistantItem(item) => {
                    if let Some(ref mut cb) = cb {
                        match item {
                            StreamedAssistantContent::Text(Text { text }) => {
                                cb(StreamEvent::Text(text)).await?;
                            },
                            StreamedAssistantContent::Reasoning(reasoning) => {
                                for rc in reasoning.content {
                                    let text = match rc {
                                        ReasoningContent::Text { text, .. } => text,
                                        ReasoningContent::Encrypted(s) => s,
                                        ReasoningContent::Redacted { data } => data,
                                        ReasoningContent::Summary(s) => s,
                                        _ => continue,
                                    };
                                    cb(StreamEvent::Reasoning(text)).await?;
                                }
                            },
                            StreamedAssistantContent::ReasoningDelta { reasoning, .. } => {
                                cb(StreamEvent::Reasoning(reasoning)).await?;
                            },
                            StreamedAssistantContent::ToolCall { tool_call, .. } => {
                                cb(StreamEvent::ToolCall {
                                    id: tool_call.id,
                                    name: tool_call.function.name,
                                    arguments: tool_call.function.arguments.to_string(),
                                })
                                .await?;
                            },
                            _ => {},
                        }
                    }
                },
                MultiTurnStreamItem::StreamUserItem(StreamedUserContent::ToolResult {
                    tool_result,
                    ..
                }) => {
                    if let Some(ref mut cb) = cb {
                        let content = tool_result
                            .content
                            .iter()
                            .filter_map(|c| match c {
                                ToolResultContent::Text(text) => Some(text.text.as_str()),
                                _ => None,
                            })
                            .collect::<Vec<_>>()
                            .join("\n");
                        cb(StreamEvent::ToolResult { id: tool_result.id.clone(), content }).await?;
                    }
                },
                MultiTurnStreamItem::FinalResponse(response) => {
                    if let Some(ref mut cb) = cb {
                        cb(StreamEvent::Finish).await?;
                    }
                    final_res = response;
                },
                _ => {},
            }
        }

        if let Some(his) = final_res.history() {
            self.history = his.to_vec();
        }

        Ok(())
    }

    pub async fn force_compact(&mut self) -> anyhow::Result<()> {
        let compressed = match &mut self.provider_agents {
            ProviderAgents::OpenAICompatible { compaction_agent, .. } => {
                compaction_agent.force_compact(&self.history, &self.transcript_dir).await?
            },
            ProviderAgents::Azure { compaction_agent, .. } => {
                compaction_agent.force_compact(&self.history, &self.transcript_dir).await?
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
