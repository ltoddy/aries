use std::future::{Future, Ready};
use std::path::PathBuf;

use anyhow::Context;
use aries_config::AriesConfig;
use aries_context::GlobalContext;
use aries_core::compaction::CompactionAgent;
use aries_core::task_spawner::{NotificationReceiver, TaskSpawner};
use aries_core::{AgentType, AgentWrapper};
use futures::{StreamExt, pin_mut};
use rig::agent::{FinalResponse, MultiTurnStreamItem, PromptHook, Text};
use rig::completion::Message;
use rig::message::{ReasoningContent, ToolResultContent};
use rig::providers::{azure, openai};
use rig::streaming::{StreamedAssistantContent, StreamedUserContent};
use time::OffsetDateTime;

pub type NoCb = fn(StreamEvent) -> Ready<anyhow::Result<()>>;

pub enum StreamEvent {
    Text(String),
    Reasoning(String),
    ToolCall { id: String, name: String, arguments: String },
    ToolResult { id: String, content: String },
}

pub enum Session<P = ()>
where
    P: PromptHook<openai::CompletionModel> + PromptHook<azure::CompletionModel>,
{
    OpenAICompatible {
        id: String,
        agent: AgentWrapper<openai::CompletionModel, P>,
        compaction_agent: CompactionAgent<openai::CompletionModel>,
        task_notifications: NotificationReceiver,
        history: Vec<Message>,
        base_history_len: usize,
        transcript_dir: PathBuf,
    },
    Azure {
        id: String,
        agent: AgentWrapper<azure::CompletionModel, P>,
        compaction_agent: CompactionAgent<azure::CompletionModel>,
        task_notifications: NotificationReceiver,
        history: Vec<Message>,
        base_history_len: usize,
        transcript_dir: PathBuf,
    },
}

impl<P> Session<P>
where
    P: PromptHook<openai::CompletionModel> + PromptHook<azure::CompletionModel> + Clone + 'static,
{
    pub fn new(id: String, context: &GlobalContext, config: AriesConfig, task_hook: P) -> anyhow::Result<Self> {
        let today = OffsetDateTime::now_utc().date();
        let history = vec![Message::user(format!("当前目录：{}\n当前日期：{}", context.current_dir.display(), today))];
        let base_history_len = history.len();
        let transcript_dir = context.config_dir.join("transcripts");

        match config.clone() {
            AriesConfig::OpenAICompatible(ref conf) => {
                let client = openai::CompletionsClient::builder()
                    .base_url(&conf.base_url)
                    .api_key(&conf.api_key)
                    .build()
                    .with_context(|| "Failed to create llm client")?;

                let (spawner, task_notifications) = TaskSpawner::new();
                let agent = AgentWrapper::new(
                    client,
                    format!("Session Agent {}", id),
                    config.clone(),
                    AgentType::Build,
                    task_hook,
                    spawner,
                );
                let compaction_agent = CompactionAgent::<openai::CompletionModel>::new(config)?;
                Ok(Self::OpenAICompatible {
                    id,
                    agent,
                    compaction_agent,
                    task_notifications,
                    history,
                    base_history_len,
                    transcript_dir,
                })
            },
            AriesConfig::Azure(ref conf) => {
                let client = azure::Client::builder()
                    .api_key(&conf.api_key)
                    .azure_endpoint(conf.azure_endpoint.to_owned())
                    .api_version(&conf.api_version)
                    .build()
                    .with_context(|| "Failed to create llm client")?;

                let (spawner, task_notifications) = TaskSpawner::new();
                let agent = AgentWrapper::new(
                    client,
                    format!("Session Agent {}", id),
                    config.clone(),
                    AgentType::Build,
                    task_hook,
                    spawner,
                );
                let compaction_agent = CompactionAgent::<azure::CompletionModel>::new(config)?;
                Ok(Self::Azure {
                    id,
                    agent,
                    compaction_agent,
                    task_notifications,
                    history,
                    base_history_len,
                    transcript_dir,
                })
            },
        }
    }

    pub fn id(&self) -> &str {
        match self {
            Self::OpenAICompatible { id, .. } => id,
            Self::Azure { id, .. } => id,
        }
    }

    pub fn history(&self) -> &[Message] {
        match self {
            Self::OpenAICompatible { history, .. } => history,
            Self::Azure { history, .. } => history,
        }
    }

    pub fn clear_history(&mut self) {
        match self {
            Self::OpenAICompatible { history, base_history_len, .. } => history.truncate(*base_history_len),
            Self::Azure { history, base_history_len, .. } => history.truncate(*base_history_len),
        }
    }

    pub async fn prompt<F, Fut>(&mut self, prompt: &str, mut cb: Option<F>) -> anyhow::Result<()>
    where
        F: FnMut(StreamEvent) -> Fut,
        Fut: Future<Output = anyhow::Result<()>>,
    {
        let (stream, history) = match self {
            Self::OpenAICompatible {
                agent,
                compaction_agent,
                task_notifications,
                history,
                base_history_len,
                transcript_dir,
                ..
            } => {
                drain_task_notifications(task_notifications, history);
                CompactionAgent::<openai::CompletionModel>::micro_compact(history);
                if let Some(compressed) = compaction_agent.auto_compact(history, transcript_dir).await? {
                    history.truncate(*base_history_len);
                    history.extend(compressed);
                }
                let snapshot = history.clone();
                (agent.stream_prompt(prompt, &snapshot).await, history)
            },
            Self::Azure {
                agent,
                compaction_agent,
                task_notifications,
                history,
                base_history_len,
                transcript_dir,
                ..
            } => {
                drain_task_notifications(task_notifications, history);
                CompactionAgent::<azure::CompletionModel>::micro_compact(history);
                if let Some(compressed) = compaction_agent.auto_compact(history, transcript_dir).await? {
                    history.truncate(*base_history_len);
                    history.extend(compressed);
                }
                let snapshot = history.clone();
                (agent.stream_prompt(prompt, &snapshot).await, history)
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
                MultiTurnStreamItem::StreamUserItem(StreamedUserContent::ToolResult { tool_result, .. }) => {
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
                MultiTurnStreamItem::FinalResponse(response) => final_res = response,
                _ => {},
            }
        }

        if let Some(new_history) = final_res.history() {
            *history = new_history.to_vec();
        }

        Ok(())
    }

    pub async fn force_compact(&mut self) -> anyhow::Result<()> {
        match self {
            Self::OpenAICompatible { compaction_agent, history, base_history_len, transcript_dir, .. } => {
                if let Some(compressed) = compaction_agent.force_compact(history, transcript_dir).await? {
                    history.truncate(*base_history_len);
                    history.extend(compressed);
                }
            },
            Self::Azure { compaction_agent, history, base_history_len, transcript_dir, .. } => {
                if let Some(compressed) = compaction_agent.force_compact(history, transcript_dir).await? {
                    history.truncate(*base_history_len);
                    history.extend(compressed);
                }
            },
        }
        Ok(())
    }
}

fn drain_task_notifications(receiver: &mut NotificationReceiver, history: &mut Vec<Message>) {
    let notifications = receiver.drain();
    if notifications.is_empty() {
        return;
    }

    let notif_text: String = notifications
        .iter()
        .map(|n| {
            let mut parts = format!("[task:{}] command={} exit_code={}", n.task_id, n.command, n.exit_code);
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

    history.push(Message::user(format!("<task-results>\n{}\n</task-results>", notif_text)));
    history.push(Message::assistant("Noted task results."));
}
