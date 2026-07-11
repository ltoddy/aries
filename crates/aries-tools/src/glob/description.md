通过 glob 模式查找匹配的文件路径。

- `pattern` 例如 `**/*.rs` 或 `src/**/*.ts`。
- 当 `pattern` 是绝对路径时，会先尝试相对于 `base_dir` 转换为相对模式。
- 工具递归遍历 `base_dir`，遵循 `../../../../.gitignore` 等忽略规则，并跳过隐藏文件。
- 返回相对于 `base_dir` 的匹配路径列表。
- 当需要进行多轮 glob + grep 的开放式搜索时，可以改用 `Agent` 工具委派给子智能体。
