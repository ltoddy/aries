use std::path::PathBuf;

use crate::tools::lsp::LspOperation;

#[derive(thiserror::Error, Debug)]
pub enum LspError {
    #[error("lsp operation {operation} failed: {message}")]
    OperationFailed { operation: LspOperation, message: String },

    #[error("failed to serialize call hierarchy item for {operation}: {source}")]
    SerializeCallHierarchyItem { operation: LspOperation, source: serde_json::Error },

    #[error("invalid input for {operation}: {message}")]
    InvalidInput { operation: LspOperation, message: String },

    #[error("failed to open document for lsp: {path}: {source}")]
    DidOpen { path: PathBuf, source: std::io::Error },
}

impl LspError {
    pub fn operation_failed(operation: LspOperation, message: impl Into<String>) -> Self {
        let message = message.into();

        Self::OperationFailed { operation, message }
    }

    pub fn serialize_call_hierarchy_item(
        operation: LspOperation,
        source: serde_json::Error,
    ) -> Self {
        Self::SerializeCallHierarchyItem { operation, source }
    }

    pub fn invalid_input(operation: LspOperation, message: impl Into<String>) -> Self {
        let message = message.into();

        Self::InvalidInput { operation, message }
    }

    pub fn did_open(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        let path = path.into();

        Self::DidOpen { path, source }
    }
}
