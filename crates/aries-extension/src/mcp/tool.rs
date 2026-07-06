use rig_core::completion::ToolDefinition;
use rig_core::tool::rmcp::McpTool;
use rig_core::tool::{ToolDyn, ToolError};
use rig_core::wasm_compat::WasmBoxedFuture;

pub struct NamespacedMcpTool {
    prefix: String,
    inner: McpTool,
}

impl NamespacedMcpTool {
    pub fn new(server_name: impl Into<String>, inner: McpTool) -> Self {
        let server_name = server_name.into();
        let name = inner.name();
        let prefix = format!("mcp__{server_name}__{name}");

        Self { prefix, inner }
    }
}

impl ToolDyn for NamespacedMcpTool {
    fn name(&self) -> String {
        self.prefix.clone()
    }

    fn definition(&self, prompt: String) -> WasmBoxedFuture<'_, ToolDefinition> {
        Box::pin(async move {
            let mut definition = self.inner.definition(prompt).await;
            definition.name.clone_from(&self.prefix);
            definition
        })
    }

    fn call(&self, args: String) -> WasmBoxedFuture<'_, Result<String, ToolError>> {
        self.inner.call(args)
    }
}
