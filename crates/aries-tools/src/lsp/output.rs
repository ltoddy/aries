use aries_lspclient::{DocumentSymbolItem, LspResult};
use serde::{Deserialize, Serialize};

use crate::{RenderError, ToolOutputRender};

#[derive(Debug, Deserialize, Serialize)]
pub struct LspOutput {
    pub result: LspResult,
}

impl ToolOutputRender for LspOutput {
    fn render_output(raw: &str) -> Result<String, RenderError> {
        let output: Self = serde_json::from_str(raw)?;
        let content = match &output.result {
            LspResult::Definition(locations)
            | LspResult::References(locations)
            | LspResult::Implementation(locations) => locations
                .iter()
                .map(|loc| {
                    format!(
                        "{}:{}:{}",
                        loc.uri.strip_prefix("file://").unwrap_or(&loc.uri),
                        loc.range.start.line + 1,
                        loc.range.start.character + 1
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
            LspResult::Hover(Some(hover)) => extract_hover_text(&hover.contents),
            LspResult::Hover(None) => String::new(),
            LspResult::DocumentSymbol(symbols) => symbols
                .iter()
                .map(|s| match s {
                    DocumentSymbolItem::Flat(s) => {
                        let loc = format!(
                            "{}:{}",
                            s.location.uri.strip_prefix("file://").unwrap_or(&s.location.uri),
                            s.location.range.start.line + 1
                        );
                        format!("{} [{}] {}", s.name, s.kind, loc)
                    },
                    DocumentSymbolItem::Hierarchical(s) => {
                        format!("{} [{}] line {}", s.name, s.kind, s.range.start.line + 1)
                    },
                })
                .collect::<Vec<_>>()
                .join("\n"),
            LspResult::WorkspaceSymbol(symbols) => symbols
                .iter()
                .map(|s| {
                    let loc = format!(
                        "{}:{}",
                        s.location.uri.strip_prefix("file://").unwrap_or(&s.location.uri),
                        s.location.range.start.line + 1
                    );
                    format!("{} [{}] {}", s.name, s.kind, loc)
                })
                .collect::<Vec<_>>()
                .join("\n"),
            LspResult::PrepareCallHierarchy(items) => items
                .iter()
                .map(|item| {
                    format!(
                        "{} [{}] {}",
                        item.name,
                        item.kind,
                        item.uri.strip_prefix("file://").unwrap_or(&item.uri)
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
            LspResult::IncomingCalls(calls) => calls
                .iter()
                .map(|c| {
                    format!(
                        "{} [{}] {}",
                        c.from.name,
                        c.from.kind,
                        c.from.uri.strip_prefix("file://").unwrap_or(&c.from.uri)
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
            LspResult::OutgoingCalls(calls) => calls
                .iter()
                .map(|c| {
                    format!(
                        "{} [{}] {}",
                        c.to.name,
                        c.to.kind,
                        c.to.uri.strip_prefix("file://").unwrap_or(&c.to.uri)
                    )
                })
                .collect::<Vec<_>>()
                .join("\n"),
        };
        Ok(content)
    }
}

fn extract_hover_text(contents: &serde_json::Value) -> String {
    match contents {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Object(obj) => {
            obj.get("value").and_then(|v| v.as_str()).unwrap_or_default().to_owned()
        },
        serde_json::Value::Array(arr) => arr
            .iter()
            .filter_map(|item| match item {
                serde_json::Value::String(s) => Some(s.as_str()),
                serde_json::Value::Object(obj) => obj.get("value").and_then(|v| v.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}
