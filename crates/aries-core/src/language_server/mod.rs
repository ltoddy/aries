mod client;
mod detection;
mod schema;

use std::path::Path;
use std::sync::Arc;

use tracing::error;

use crate::fs::path_to_uri;
pub use crate::language_server::client::{DocumentSymbolItem, LspClient, LspResult};
pub use crate::language_server::detection::LspServerInfo;
pub use crate::language_server::schema::{
    CallHierarchyIncomingCall, CallHierarchyItem, CallHierarchyOutgoingCall, DocumentSymbol, Hover,
    Location, Position, Range, SymbolInformation, SymbolKind,
};

pub type SharedLspClient = Arc<LspClient>;

pub async fn warm_up(
    info: LspServerInfo,
    project_dir: impl AsRef<Path>,
) -> anyhow::Result<SharedLspClient> {
    let lsp = LspClient::start(info.clone()).await.map_err(|err| {
        error!("Failed to start {}: {err}", info.binary);
        err
    })?;

    let root_uri = path_to_uri(project_dir.as_ref());
    let shared = Arc::new(lsp);

    let init = shared.clone();
    tokio::task::spawn(async move {
        if let Err(err) = init.initialize(&root_uri).await {
            error!("Failed to initialize language server: {err}")
        }
    });

    Ok(shared)
}
