# LSP 集成

- 自动检测当前项目语言对应的 LSP 服务器并预热（`warm_up`）。
- 提供 `lsp` 工具供 Agent 调用（定义跳转、引用查找、悬停、符号列表、调用层次等）。
- `write` / `edit` / `multiedit` 写入后通知 LSP 重建索引。
