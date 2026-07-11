use rig_core::client::CompletionClient;
use rig_core::completion::CompletionModel;
use rig_core::extractor::Extractor;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::MemoryType;

const PREAMBLE: &str = include_str!("preamble.md");

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExtractedMemories {
    pub memories: Vec<ExtractedMemory>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExtractedMemory {
    pub name: String,
    pub description: String,
    #[serde(rename = "type")]
    pub memory_type: MemoryType,
    pub body: String,
}

pub struct MemoryExtractor<M: CompletionModel> {
    inner: Extractor<M, ExtractedMemories>,
}

impl<M: CompletionModel> MemoryExtractor<M> {
    pub fn new<C>(client: C, model: impl Into<String>) -> Self
    where
        C: CompletionClient<CompletionModel = M>,
    {
        let inner = client.extractor::<ExtractedMemories>(model).preamble(PREAMBLE).build();

        Self { inner }
    }

    pub async fn extract(
        &self,
        user: impl Into<String>,
        assistant: impl Into<String>,
    ) -> Vec<ExtractedMemory> {
        let user = user.into();
        let assistant = assistant.into();

        let prompt = format!("<user>\n{user}\n</user>\n<assistant>\n{assistant}\n</assistant>");

        match self.inner.extract(&prompt).await {
            Ok(result) => result.memories,
            Err(_) => Vec::new(),
        }
    }
}
