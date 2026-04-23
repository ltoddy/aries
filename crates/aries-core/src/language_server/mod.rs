mod client;
mod detection;

use std::path::Path;
use std::sync::Arc;

pub use self::client::LspClient;
pub use self::detection::LspServerInfo;

pub type SharedLspClient = Arc<LspClient>;

pub async fn warm_up(info: LspServerInfo, project_dir: &Path) -> anyhow::Result<SharedLspClient> {
    let lsp = LspClient::start(info.clone()).await.map_err(|err| {
        eprintln!("Failed to start {}: {err}", info.binary);
        err
    })?;

    let root_uri = format!("file://{}", project_dir.display());
    let shared = Arc::new(lsp);

    let init = shared.clone();
    tokio::task::spawn(async move {
        if let Err(err) = init.initialize(&root_uri).await {
            eprintln!("Failed to initialize language server: {err}")
        }
    });

    Ok(shared)
}
