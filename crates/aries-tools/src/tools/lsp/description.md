查询当前项目语言服务器提供的符号定义、引用、类型和调用关系，用于需要理解代码语义的定位与分析。字面文本搜索用 `Grep`，按文件名找路径用 `Glob`；本工具不编辑代码。

按需求选择 `operation`：
- `goToDefinition` / `goToImplementation`：返回定义或实现位置。
- `findReferences`：返回服务器找到的引用位置，包括声明。
- `hover`：返回类型、签名或文档信息。
- `documentSymbol`：返回指定文件中的符号。
- `workspaceSymbol`：按 `query` 在语言服务器的工作区中搜索符号，查询规则由服务器决定。
- `prepareCallHierarchy`：返回指定位置的调用层次入口项。
- `incomingCalls` / `outgoingCalls`：返回调用方 / 被调用方；工具会自动准备调用层次，并只查询首个入口项。

参数与限制：
- 除 `workspaceSymbol` 外都需要 `file_path`，支持绝对路径或相对于当前工作目录的路径。除 `documentSymbol`、`workspaceSymbol` 外还必须提供 `line` 和 `character`，指向目标符号。
- 输入坐标从 0 开始；`line` 是行号，`character` 是行内 UTF-16 代码单元偏移，不是字节偏移或视觉列。从 `Read` 的行号换算时减 1，不计展示前缀。
- `query` 仅用于 `workspaceSymbol`，省略时传空串；其他操作不使用它。`documentSymbol` 不需要位置坐标。
- 返回 `result`，结构随操作变化，可能是位置列表、悬停信息、符号或调用关系；原始位置坐标从 0 开始，格式化展示的行列号从 1 开始。空结果仅表示服务器未返回信息，不能证明符号或引用不存在。
- 需要已连接且支持相应操作的语言服务器；工具会先打开提供的文件以同步内容，但不保证索引已完整。调用失败或结果不足时，可用 `Grep` / `Read` 补充检查。
