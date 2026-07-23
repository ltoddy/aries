使用正则表达式在工作目录中搜索文件内容，基于 ripgrep 的语义。

- `pattern` 使用 Rust `regex_lite` 语法（不支持环视、反向引用等扩展特性）。
- `include` 形如 `src/**/*.rs`，用于把搜索范围限制在匹配的文件上。
- `output_mode` 控制输出形态：`content` 显示匹配的行内容，`files_with_matches` 只显示文件路径（缺省，按修改时间降序），`count` 显示每个文件的匹配计数。
- `case_insensitive` 对应 `-i`，开启大小写不敏感搜索（缺省为 false）。
- `show_line_numbers` 对应 `-n`，仅在 `content` 模式下决定是否显示行号（缺省为 true）。
- `context_before` / `context_after` / `context` 对应 `-B` / `-A` / `-C`，显示匹配行前后的上下文行，仅在 `content` 模式下生效；`context` 优先于前两者。
- `head_limit` 限制返回的行数/条目数（缺省 250，传 0 表示不限制）；超出上限时会截断并给出提示。
- 工具会以工作目录为根，遵循 `.gitignore` 等忽略规则递归查找文件，但不会跳过隐藏文件/目录（`hidden(false)`）。
- 当需要进行多轮交叉的 glob + grep 搜索时，可以改用 `Agent` 工具委派给子智能体。
