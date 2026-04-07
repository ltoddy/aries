use aries_config::AriesConfig;
use aries_context::GlobalContext;
use aries_core::AgentWrapper;
use aries_core::agent_type::AgentType;
use aries_core::compaction::CompactionAgent;
use rig::agent::{FinalResponse, PromptHook, StreamingResult};
use rig::completion::{self, Message};
use rig::providers::openai;

pub struct Session<P = ()>
where
    P: PromptHook<openai::CompletionModel>,
{
    id: String,
    agent: AgentWrapper<openai::CompletionModel, P>,
    compaction_agent: CompactionAgent<openai::CompletionModel>,
    history: Vec<Message>,
    base_history_len: usize,
}

impl Session<()> {
    pub fn new(id: String, context: &GlobalContext, config: AriesConfig) -> anyhow::Result<Self> {
        let history = vec![Message::user(format!("当前目录：{}", context.current_dir.display()))];
        let base_history_len = history.len();
        let agent = AgentWrapper::<openai::CompletionModel, ()>::new(
            format!("Session Agent {}", id),
            config.clone(),
            AgentType::Orchestrate,
            (),
        )?;
        let compaction_agent = CompactionAgent::<openai::CompletionModel>::new(config.clone())?;

        Ok(Self { id, agent, compaction_agent, history, base_history_len })
    }
}

impl<P> Session<P>
where
    P: PromptHook<openai::CompletionModel> + Clone + 'static,
{
    pub fn new_with_task_hook(
        id: String,
        context: &GlobalContext,
        config: AriesConfig,
        task_hook: P,
    ) -> anyhow::Result<Self> {
        let history = vec![Message::user(format!("当前目录：{}", context.current_dir.display()))];
        let base_history_len = history.len();
        let agent = AgentWrapper::<openai::CompletionModel, P>::new(
            format!("Session Agent {}", id),
            config.clone(),
            AgentType::Orchestrate,
            task_hook,
        )?;
        let compaction_agent = CompactionAgent::<openai::CompletionModel>::new(config.clone())?;

        Ok(Self { id, agent, compaction_agent, history, base_history_len })
    }

    pub async fn stream_prompt(
        &mut self,
        prompt: &str,
    ) -> StreamingResult<<openai::CompletionModel as completion::CompletionModel>::StreamingResponse> {
        let _ = self.compact_if_needed().await;
        let history = self.history.clone();
        self.agent.stream_prompt(prompt, &history).await
    }

    pub fn update_history_from_stream(&mut self, response: &FinalResponse) {
        self.update_history_from_response(response);
    }

    pub async fn compact_if_needed(&mut self) -> anyhow::Result<Option<String>> {
        let Some(summary) = self.compaction_agent.compact(self.history.clone()).await? else {
            return Ok(None);
        };

        // 清理 session 的历史并添加摘要
        self.history.truncate(self.base_history_len);
        self.history.push(Message::assistant(summary.clone()));
        // 不需要同步到 agent，因为 agent 不再维护自己的历史

        Ok(Some(summary))
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn history(&self) -> &[Message] {
        &self.history
    }

    pub fn clear_history(&mut self) {
        self.history.truncate(self.base_history_len);
    }

    fn update_history_from_response(&mut self, response: &FinalResponse) {
        if let Some(history) = response.history() {
            self.history = history.to_vec();
        }
    }
}
