use rig_core::tool::rmcp::McpTool;
use rig_core::tool::{ToolDyn, ToolError};
use rig_core::wasm_compat::WasmBoxedFuture;
use serde_json::Value;

pub struct NamespacedMcpTool {
    name: String,
    inner: McpTool,
}

impl NamespacedMcpTool {
    pub fn new(server_name: impl Into<String>, inner: McpTool) -> Self {
        let server_name = server_name.into();
        let name = inner.name();
        let name = format!("mcp__{server_name}__{name}");

        Self { name, inner }
    }
}

impl ToolDyn for NamespacedMcpTool {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn description(&self) -> String {
        self.inner.description()
    }

    fn parameters(&self) -> Value {
        self.inner.parameters()
    }

    fn call(&self, args: String) -> WasmBoxedFuture<'_, Result<String, ToolError>> {
        self.inner.call(args)
    }
}
