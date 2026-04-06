use std::ops::{Deref, DerefMut};

use aries_config::AriesConfig;
use aries_context::GlobalContext;
use rig::completion::Message;

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
