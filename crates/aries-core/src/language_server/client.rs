use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::oneshot;

use crate::fs::path_to_uri;
use crate::language_server::{
    CallHierarchyIncomingCall, CallHierarchyItem, CallHierarchyOutgoingCall, Hover, Location,
    LspServerInfo, SymbolInformation,
};
use crate::rpc::{JsonRpcMessage, Notification, Request, RequestId, Response};

#[derive(Debug, Deserialize, Serialize)]
pub enum LspResult {
    Definition(Vec<Location>),
    References(Vec<Location>),
    Hover(Hover),
    DocumentSymbol(Vec<SymbolInformation>),
    WorkspaceSymbol(Vec<SymbolInformation>),
    Implementation(Vec<Location>),
    PrepareCallHierarchy(Vec<CallHierarchyItem>),
    IncomingCalls(Vec<CallHierarchyIncomingCall>),
    OutgoingCalls(Vec<CallHierarchyOutgoingCall>),
}

pub struct LspClient {
    stdin: tokio::sync::Mutex<tokio::process::ChildStdin>,
    _child: Child,
    next_id: AtomicI64,
    pending: Arc<Mutex<HashMap<RequestId, oneshot::Sender<Value>>>>,
}

impl LspClient {
    pub async fn start(info: LspServerInfo) -> anyhow::Result<Self> {
        let mut child = Command::new(info.binary)
            .args(info.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to open stdin for LSP process"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to open stdout for LSP process"))?;

        let pending: Arc<Mutex<HashMap<RequestId, oneshot::Sender<Value>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let pending_clone = pending.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut header = String::new();
                if reader.read_line(&mut header).await.unwrap_or(0) == 0 {
                    break;
                }

                let content_length: usize =
                    if let Some(len_str) = header.strip_prefix("Content-Length: ") {
                        len_str.trim().parse().unwrap_or(0)
                    } else {
                        continue;
                    };

                let mut empty_line = String::new();
                let _ = reader.read_line(&mut empty_line).await;

                let mut body = vec![0u8; content_length];
                if reader.read_exact(&mut body).await.is_err() {
                    break;
                }

                let Ok(response) = serde_json::from_slice::<JsonRpcMessage<Response>>(&body) else {
                    continue;
                };

                let (id, result) = response.message.into_result();
                let value = result.unwrap_or_else(|e| serde_json::json!({ "error": e.message }));

                if let Some(tx) = pending_clone.lock().remove(&id) {
                    let _ = tx.send(value);
                }
            }
        });

        Ok(Self {
            stdin: tokio::sync::Mutex::new(stdin),
            _child: child,
            next_id: AtomicI64::new(1),
            pending,
        })
    }

    pub async fn initialize(&self, root_uri: &str) -> anyhow::Result<Value> {
        let params = serde_json::json!({
            "processId": std::process::id(),
            "rootUri": root_uri,
            "capabilities": {
                "textDocument": {
                    "definition": { "dynamicRegistration": false },
                    "references": { "dynamicRegistration": false },
                    "hover": { "dynamicRegistration": false },
                    "documentSymbol": { "dynamicRegistration": false },
                    "implementation": { "dynamicRegistration": false },
                    "callHierarchy": { "dynamicRegistration": false }
                },
                "workspace": {
                    "symbol": { "dynamicRegistration": false }
                }
            }
        });

        let result = self.send_request("initialize", params).await?;
        self.send_notification("initialized", serde_json::json!({})).await?;
        Ok(result)
    }

    pub async fn send_request(&self, method: &str, params: Value) -> anyhow::Result<Value> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let request = JsonRpcMessage::wrap(Request::new(id, method, params));

        let body = serde_json::to_string(&request)?;
        let message = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);

        let (tx, rx) = oneshot::channel();
        self.pending.lock().insert(RequestId::Number(id), tx);

        {
            let mut stdin = self.stdin.lock().await;
            stdin.write_all(message.as_bytes()).await?;
            stdin.flush().await?;
        }

        let result = tokio::time::timeout(std::time::Duration::from_secs(30), rx).await??;
        Ok(result)
    }

    pub async fn send_notification(&self, method: &str, params: Value) -> io::Result<()> {
        let notification = JsonRpcMessage::wrap(Notification::new(method, params));
        let body = serde_json::to_string(&notification)?;
        let message = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);
        let mut stdin = self.stdin.lock().await;
        stdin.write_all(message.as_bytes()).await?;
        stdin.flush().await?;
        Ok(())
    }

    pub async fn goto_definition(
        &self,
        file_path: impl AsRef<Path>,
        line: u32,
        character: u32,
    ) -> anyhow::Result<LspResult> {
        let params = text_document_position_params(file_path, line, character);
        let result = self.send_request("textDocument/definition", params).await?;
        let locations: Vec<Location> = serde_json::from_value(result)?;
        Ok(LspResult::Definition(locations))
    }

    pub async fn find_references(
        &self,
        file_path: impl AsRef<Path>,
        line: u32,
        character: u32,
    ) -> anyhow::Result<LspResult> {
        let mut params = text_document_position_params(file_path, line, character);
        params["context"] = serde_json::json!({ "includeDeclaration": true });
        let result = self.send_request("textDocument/references", params).await?;
        let locations: Vec<Location> = serde_json::from_value(result)?;
        Ok(LspResult::References(locations))
    }

    pub async fn hover(
        &self,
        file_path: impl AsRef<Path>,
        line: u32,
        character: u32,
    ) -> anyhow::Result<LspResult> {
        let params = text_document_position_params(file_path, line, character);
        let result = self.send_request("textDocument/hover", params).await?;
        let hover: Hover = serde_json::from_value(result)?;
        Ok(LspResult::Hover(hover))
    }

    pub async fn document_symbol(&self, file_path: impl AsRef<Path>) -> anyhow::Result<LspResult> {
        let params = serde_json::json!({
            "textDocument": {
                "uri": path_to_uri(file_path)
            }
        });
        let result = self.send_request("textDocument/documentSymbol", params).await?;
        let symbols: Vec<SymbolInformation> = serde_json::from_value(result)?;
        Ok(LspResult::DocumentSymbol(symbols))
    }

    pub async fn workspace_symbol(&self, query: &str) -> anyhow::Result<LspResult> {
        let params = serde_json::json!({ "query": query });
        let result = self.send_request("workspace/symbol", params).await?;
        let symbols: Vec<SymbolInformation> = serde_json::from_value(result)?;
        Ok(LspResult::WorkspaceSymbol(symbols))
    }

    pub async fn goto_implementation(
        &self,
        file_path: impl AsRef<Path>,
        line: u32,
        character: u32,
    ) -> anyhow::Result<LspResult> {
        let params = text_document_position_params(file_path, line, character);
        let result = self.send_request("textDocument/implementation", params).await?;
        let locations: Vec<Location> = serde_json::from_value(result)?;
        Ok(LspResult::Implementation(locations))
    }

    pub async fn prepare_call_hierarchy(
        &self,
        file_path: impl AsRef<Path>,
        line: u32,
        character: u32,
    ) -> anyhow::Result<LspResult> {
        let params = text_document_position_params(file_path, line, character);
        let result = self.send_request("textDocument/prepareCallHierarchy", params).await?;
        let items: Vec<CallHierarchyItem> = serde_json::from_value(result)?;
        Ok(LspResult::PrepareCallHierarchy(items))
    }

    pub async fn incoming_calls(&self, item: Value) -> anyhow::Result<LspResult> {
        let params = serde_json::json!({ "item": item });
        let result = self.send_request("callHierarchy/incomingCalls", params).await?;
        let calls: Vec<CallHierarchyIncomingCall> = serde_json::from_value(result)?;
        Ok(LspResult::IncomingCalls(calls))
    }

    pub async fn outgoing_calls(&self, item: Value) -> anyhow::Result<LspResult> {
        let params = serde_json::json!({ "item": item });
        let result = self.send_request("callHierarchy/outgoingCalls", params).await?;
        let calls: Vec<CallHierarchyOutgoingCall> = serde_json::from_value(result)?;
        Ok(LspResult::OutgoingCalls(calls))
    }

    pub async fn did_open(&self, file_path: impl AsRef<Path>) -> io::Result<()> {
        let content = tokio::fs::read_to_string(file_path.as_ref()).await?;
        let language_id = detect_language_id(file_path.as_ref());
        let params = serde_json::json!({
            "textDocument": {
                "uri": path_to_uri(file_path),
                "languageId": language_id,
                "version": 1,
                "text": content
            }
        });
        self.send_notification("textDocument/didOpen", params).await
    }

    pub async fn shutdown(&self) -> anyhow::Result<()> {
        let _ = self.send_request("shutdown", Value::Null).await;
        self.send_notification("exit", Value::Null).await?;
        Ok(())
    }
}

fn text_document_position_params(file_path: impl AsRef<Path>, line: u32, character: u32) -> Value {
    serde_json::json!({
        "textDocument": {
            "uri": path_to_uri(file_path)
        },
        "position": {
            "line": line.saturating_sub(1),
            "character": character.saturating_sub(1)
        }
    })
}

fn detect_language_id(path: impl AsRef<Path>) -> &'static str {
    match path.as_ref().extension().and_then(|e| e.to_str()) {
        Some("rs") => "rust",
        Some("ts") => "typescript",
        Some("tsx") => "typescriptreact",
        Some("js") => "javascript",
        Some("jsx") => "javascriptreact",
        Some("py") => "python",
        Some("go") => "go",
        Some("c") => "c",
        Some("cpp" | "cc" | "cxx") => "cpp",
        Some("h" | "hpp") => "cpp",
        Some("rb") => "ruby",
        Some("swift") => "swift",
        _ => "plaintext",
    }
}
