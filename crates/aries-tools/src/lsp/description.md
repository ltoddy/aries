与语言服务器协议 (LSP) 服务器交互以获取代码智能能力。

支持的 `operation`：
- `goToDefinition`：查找符号定义位置
- `findReferences`：查找符号的所有引用
- `hover`：获取符号的悬停信息（文档、类型）
- `documentSymbol`：列出文件中的所有符号
- `workspaceSymbol`：在整个工作区中按 `query` 搜索符号
- `goToImplementation`：查找接口/抽象方法的实现
- `prepareCallHierarchy`：在某位置准备调用层次入口项
- `incomingCalls`：查找调用某位置函数的调用方（基于该位置自动 prepareCallHierarchy 的首个 item）
- `outgoingCalls`：查找某位置函数所调用的目标（基于该位置自动 prepareCallHierarchy 的首个 item）

参数要求：
- 位置类操作（goToDefinition、findReferences、hover、goToImplementation、prepareCallHierarchy、incomingCalls、outgoingCalls）以及 `documentSymbol` 都需要 `file_path`；相对路径会基于当前工作目录解析。
- `line` / `character` 是从 0 开始的位置坐标，位置类操作必需。
- `query` 仅 `workspaceSymbol` 使用，缺省视为空串。

调用前若提供了 `file_path`，工具会先发送 `didOpen` 通知服务器加载该文档。底层会根据当前项目语言自动选择 LSP 服务器（如 rust-analyzer、typescript-language-server、gopls 等），若未安装会以错误返回。
