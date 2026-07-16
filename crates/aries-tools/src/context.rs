use std::path::Path;

use aries_lspclient::SharedLspClient;

#[derive(Clone, Default)]
pub struct ToolContext {
    pub lsp_client: Option<SharedLspClient>,
}

impl std::fmt::Debug for ToolContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolContext").field("has_lsp_client", &self.lsp_client.is_some()).finish()
    }
}

impl ToolContext {
    pub fn new(lsp_client: Option<SharedLspClient>) -> Self {
        Self { lsp_client }
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
