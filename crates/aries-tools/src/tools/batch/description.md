在一次调用中并发执行多个独立的工具调用以减少延迟。

负载示例：
```json
{
  "calls": [
    { "tool": "Read",  "parameters": { "file_path": "src/index.ts", "offset": 1 } },
    { "tool": "Grep",  "parameters": { "pattern": "Session\\.updatePart", "include": "src/**/*.ts" } },
    { "tool": "Glob",  "parameters": { "pattern": "src/**/*.ts" } }
  ]
}
```

执行规则：
- 每次调用最多 25 个 call，超过会整个调用失败。
- 全部 call 并行启动，不保证顺序；单个失败不会影响其他 call。
- 每个 call 返回 `{ "success": true, "result": ... }` 或 `{ "success": false, "error": ... }`。
- 支持作为子工具的列表：`Agent`、`Read`、`Glob`、`Grep`、`WebFetch`、`WebSearch`、`CodeSearch`、`TaskOutput`。
- 写操作、进程控制和嵌套 `Batch` 在 batch 中调用都会被拒绝（返回 `success: false` 与对应错误消息）。

适合的场景：
- 一次性读取多个文件；
- 组合 `Glob` + `Grep` + `Read`；
- 多个互不依赖的只读查询；
- 多个互不依赖的 `Agent` 调研任务。

不适合的场景：
- 后一步依赖前一步结果（例如先创建文件再读取）——请改用顺序调用。
- 顺序敏感的有状态变更。
