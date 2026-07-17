mod file_checkpoint;

use std::path::Path;

use aries_lspclient::SharedLspClient;

use crate::context::file_checkpoint::SharedFileCheckpoint;

#[derive(Clone)]
pub struct ToolContext {
    lsp_client: Option<SharedLspClient>,
    pub file_checkpoint: SharedFileCheckpoint,
}

impl std::fmt::Debug for ToolContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolContext").field("has_lsp_client", &self.lsp_client.is_some()).finish()
    }
}

impl ToolContext {
    pub fn new(lsp_client: Option<SharedLspClient>) -> Self {
        let file_checkpoint = SharedFileCheckpoint::new();

        Self { lsp_client, file_checkpoint }
    }

    pub async fn on_file_written(&self, file_path: impl AsRef<Path>, content: impl Into<String>) {
        let file_path = file_path.as_ref();
        let content = content.into();

        if let Some(client) = &self.lsp_client {
            let _ = client.did_change(file_path, &content).await;
            let _ = client.did_save(file_path, &content).await;
        }
    }
}
