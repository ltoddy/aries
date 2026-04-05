use std::time::Instant;

use aries_context::GlobalContext;
use rig::completion::Message;
use rig::providers::openai;

use crate::compaction::CompactionAgent;
use crate::{AgentType, AgentWrapper, create};

pub struct OrchestrateAgent {
    inner: AgentWrapper<openai::CompletionModel>,
    history: Vec<Message>,
    compaction_agent: CompactionAgent<openai::CompletionModel>,
}

impl OrchestrateAgent {
    pub fn new(context: GlobalContext) -> anyhow::Result<Self> {
        let agent = create(context.clone(), AgentType::Orchestrate)?;
        let name = env!("CARGO_PKG_NAME").to_owned();
        let history = vec![Message::user(format!("当前目录：{}", context.current_dir.display()))];
        let inner = AgentWrapper::new(name, agent);
        let compaction_agent = CompactionAgent::new(create(context.clone(), AgentType::Compaction)?);

        Ok(Self { inner, history, compaction_agent })
    }

    #[inline]
    pub fn clear_history(&mut self) {
        self.history.truncate(1)
    }

    #[inline]
    pub fn chat_history(&self) -> &[Message] {
        &self.history
    }

    pub async fn completion(&mut self, input: &str) -> anyhow::Result<()> {
        let start = Instant::now();
        let theme = aries_context::Theme::default();

        let final_res = self.inner.completion(input, self.history.clone()).await?;
        if let Some(history) = final_res.history() {
            self.history = history.to_vec()
        }

        let elapsed = start.elapsed();
        println!("{}", theme.dimmed(&format!("⏱️  耗时: {:.2}s", elapsed.as_secs_f64())));

        let messages = self.history.clone();
        if let Ok(Some(summary)) = self.compaction_agent.compact(messages).await {
            self.clear_history();
            self.history.push(Message::assistant(summary));
        }

        Ok(())
    }
}
