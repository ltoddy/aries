通过 Exa 的远程 MCP 服务（`https://mcp.exa.ai/mcp`）检索第三方库、SDK、API、框架的代码示例与文档上下文。**用于查询外部依赖与编程概念，而不是搜索本地代码库**——本地搜索请使用 `Grep` / `Glob`。

- 以 JSON-RPC 形式调用 Exa MCP 的 `get_code_context_exa`，解析 SSE 响应并返回首个内容片段的文本。
- 如果未匹配到内容，会返回一段提示，建议换个查询、明确库名或检查拼写。
