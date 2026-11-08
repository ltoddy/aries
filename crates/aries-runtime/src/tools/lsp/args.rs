use std::fmt::{Display, Formatter};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LspOperation {
    GoToDefinition,
    FindReferences,
    Hover,
    DocumentSymbol,
    WorkspaceSymbol,
    GoToImplementation,
    PrepareCallHierarchy,
    IncomingCalls,
    OutgoingCalls,
}

impl Display for LspOperation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            LspOperation::GoToDefinition => write!(f, "goToDefinition"),
            LspOperation::FindReferences => write!(f, "findReferences"),
            LspOperation::Hover => write!(f, "hover"),
            LspOperation::DocumentSymbol => write!(f, "documentSymbol"),
            LspOperation::WorkspaceSymbol => write!(f, "workspaceSymbol"),
            LspOperation::GoToImplementation => write!(f, "goToImplementation"),
            LspOperation::PrepareCallHierarchy => write!(f, "prepareCallHierarchy"),
            LspOperation::IncomingCalls => write!(f, "incomingCalls"),
            LspOperation::OutgoingCalls => write!(f, "outgoingCalls"),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LspArgs {
    pub operation: LspOperation,
    pub file_path: Option<PathBuf>,
    pub line: Option<u32>,
    pub character: Option<u32>,
    pub query: Option<String>,
}

impl LspArgs {
    pub fn title(&self) -> String {
        match (&self.file_path, &self.query) {
            (Some(path), _) => format!("Run {} on {}", self.operation, path.display()),
            (None, Some(query)) => format!("Run {} for {}", self.operation, query),
            (None, None) => format!("Run {}", self.operation),
        }
    }
}

impl LspArgs {
    pub fn render_args(raw: &str) -> Result<(String, Option<String>), serde_json::Error> {
        let args: Self = serde_json::from_str(raw)?;

        let mut first = format!("{}", args.operation);
        if let Some(path) = args.file_path {
            first.push_str(&format!(" {}", path.display()));
        }
        if let Some(line) = args.line {
            first.push_str(&format!(":{line}"));
        }
        if let Some(character) = args.character {
            first.push_str(&format!(":{character}"));
        }
        if let Some(query) = args.query {
            first.push_str(&format!(" query = {query}"));
        }

        Ok((first, None))
    }
}
