// use rig_agent::tool::DynamicTool;
// use rig_agent::tool::rmcp::McpClientHandler;
// use rmcp::handler::server::tool::CallToolHandler;
//
// pub struct NamespacedMcpTool {
//     name: String,
//     inner: McpClientHandler,
// }
//
// impl NamespacedMcpTool {
//     pub fn new(server_name: impl Into<String>, inner: McpClientHandler) -> Self {
//         let server_name = server_name.into();
//         let name = inner.name();
//         let name = format!("mcp__{server_name}__{name}");
//
//         Self { name, inner }
//     }
//
//     pub fn into_dynamic_tool(self) -> DynamicTool {
//         DynamicTool::new(
//             self.name,
//             self.inner.description(),
//             self.inner.parameters(),
//             async |x, value| self.inner.call(value),
//         )
//     }
// }
//
// // impl ToolDyn for NamespacedMcpTool {
// //     fn name(&self) -> String {
// //         self.name.clone()
// //     }
// //
// //     fn description(&self) -> String {
// //         self.inner.description()
// //     }
// //
// //     fn parameters(&self) -> Value {
// //         self.inner.parameters()
// //     }
// //
// //     fn call(&self, args: String) -> WasmBoxedFuture<'_, Result<String, ToolError>> {
// //         self.inner.call(args)
// //     }
// // }
