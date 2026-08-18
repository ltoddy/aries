use itertools::Itertools;
use rig::client::AgentClientExt;
use rig::extractor::Extractor;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::Memory;

const PREAMBLE: &str = include_str!("preamble.md");

const MAX_SELECTED: usize = 5;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RetrievedMemories {
    pub file_names: Vec<String>,
}

pub struct MemoryRetriever {
    inner: Extractor<RetrievedMemories>,
}

impl MemoryRetriever {
    pub fn new<C>(client: C, model: impl Into<String>) -> Self
    where
        C: AgentClientExt + 'static,
    {
        let inner = client.extractor::<RetrievedMemories>(model).preamble(PREAMBLE).build();

        Self { inner }
    }

    pub async fn retrieve(&self, query: impl Into<String>, memories: &[Memory]) -> Vec<String> {
        if memories.is_empty() {
            return vec![];
        }

        let query = query.into();
        let manifest = memories.iter().map(Memory::to_retriever_line).join("\n");
        let prompt = ["<memories>", &manifest, "</memories>", "\n", "<query>", &query, "</query>"]
            .join("\n");

        let retrieved = match self.inner.extract(&prompt).await {
            Ok(res) => res.file_names,
            Err(err) => {
                info!("failed to retrieve relevant memories: {err}");
                return vec![];
            },
        };

        retrieved
            .into_iter()
            .filter(|name| memories.iter().any(|m| &m.file_name == name))
            .take(MAX_SELECTED)
            .collect()
    }
}
