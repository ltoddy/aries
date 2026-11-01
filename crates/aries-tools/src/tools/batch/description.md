在一次调用中并发执行多个互不依赖的工具请求，用于并行读取已知文件、搜索不同范围或开展独立调研。后一个请求需要前一个结果时，应逐步调用对应工具。

- `calls` 是调用数组，每项包含工具名 `tool` 和该工具的参数对象 `parameters`；参数必须事先已知，不能引用本批次其他请求的结果。
- 最多 25 项，超过时整批拒绝执行。支持的工具名区分大小写：`Agent`、`Read`、`Glob`、`Grep`、`WebFetch`、`WebSearch`、`CodeSearch`、`TaskOutput`；其他工具和嵌套 `Batch` 会作为该项的失败返回。
- 各项并发执行，不保证执行或完成顺序；等待全部结束后，`results` 按输入顺序返回。每项为 `{ "success": true, "result": ... }` 或 `{ "success": false, "error": ... }`，单项失败不取消其他请求。
- `result` 保留子工具自身的结果格式和截断限制。整批返回不代表每项都成功，应逐项检查；重试时只重试需要重试的项。
- 本工具不提供事务或回滚。`Agent` 仍可按其模式执行命令和修改文件；只读调研须按 `Agent` 说明核对模式及自定义工具范围，避免并发修改同一文件。`TaskOutput` 默认会等待任务结束，检查长期监控任务时须显式传 `block=false`。

示例：并行读取两个已知文件。

```json
{"calls":[{"tool":"Read","parameters":{"file_path":"src/lib.rs"}},{"tool":"Read","parameters":{"file_path":"Cargo.toml"}}]}
```
