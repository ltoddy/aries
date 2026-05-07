use aries_core::language_server::LspResult;
use aries_core::tools::lsp::{LspArgs, LspOutput, NAME};

use crate::display::preview;
use crate::theme::Theme;

pub fn format_tool_call(args: &str, theme: &Theme) -> (String, Option<String>) {
    let args = serde_json::from_str::<LspArgs>(args);

    let first = match args {
        Ok(args) => {
            let mut display = format!("{:?}", args.operation);
            if let Some(path) = args.file_path {
                display.push_str(&format!(" {}", path.display()));
            }
            if let Some(line) = args.line {
                display.push_str(&format!(":{line}"));
            }
            if let Some(character) = args.character {
                display.push_str(&format!(":{character}"));
            }
            if let Some(query) = args.query {
                display.push_str(&format!(" query = {query}"));
            }
            display
        },
        Err(_) => return (String::from("?"), None),
    };

    (format!("{} {}", theme.cyan_text(NAME), theme.yellow_text(&first)), None)
}

pub fn format_tool_result(result: &str, theme: Theme) -> String {
    let output = serde_json::from_str::<LspOutput>(result);

    match output {
        Ok(output) => {
            let content = match output.result {
                LspResult::Definition(locations)
                | LspResult::References(locations)
                | LspResult::Implementation(locations) => locations
                    .iter()
                    .map(|loc| {
                        format!(
                            "{}:{}:{}",
                            strip_file_uri(&loc.uri),
                            loc.range.start.line + 1,
                            loc.range.start.character + 1
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
                LspResult::Hover(hover) => extract_hover_text(&hover.contents),
                LspResult::DocumentSymbol(symbols) | LspResult::WorkspaceSymbol(symbols) => symbols
                    .iter()
                    .map(|s| {
                        let loc = format!(
                            "{}:{}",
                            strip_file_uri(&s.location.uri),
                            s.location.range.start.line + 1
                        );
                        format!("{} [{}] {}", s.name, s.kind, loc)
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
                LspResult::PrepareCallHierarchy(items) => items
                    .iter()
                    .map(|item| {
                        format!("{} [{}] {}", item.name, item.kind, strip_file_uri(&item.uri))
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
                LspResult::IncomingCalls(calls) => calls
                    .iter()
                    .map(|c| {
                        format!("{} [{}] {}", c.from.name, c.from.kind, strip_file_uri(&c.from.uri))
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
                LspResult::OutgoingCalls(calls) => calls
                    .iter()
                    .map(|c| format!("{} [{}] {}", c.to.name, c.to.kind, strip_file_uri(&c.to.uri)))
                    .collect::<Vec<_>>()
                    .join("\n"),
            };
            theme.dimmed(&preview(content)).to_string()
        },
        Err(_) => theme.red_text(result).to_string(),
    }
}

fn strip_file_uri(uri: &str) -> &str {
    uri.strip_prefix("file://").unwrap_or(uri)
}

fn extract_hover_text(contents: &serde_json::Value) -> String {
    match contents {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Object(obj) => {
            obj.get("value").and_then(|v| v.as_str()).unwrap_or_default().to_string()
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
