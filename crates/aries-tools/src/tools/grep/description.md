使用正则表达式在工作目录中搜索文件内容，基于 ripgrep 的语义。

- `pattern` 使用 Rust `regex` 语法，逐行匹配（由 `grep-regex` 实现，不支持跨行匹配、环视、反向引用等扩展特性）。
- `include` 形如 `src/**/*.rs`，用于把搜索范围限制在匹配的文件上。
- `output_mode` 控制输出形态：`content` 显示匹配的行内容，`files_with_matches` 只显示文件路径（缺省，按修改时间降序），`count` 显示每个文件的匹配行数。
- `case_insensitive` 对应 `-i`，开启大小写不敏感搜索（缺省为 false）。
- `show_line_numbers` 对应 `-n`，仅在 `content` 模式下决定是否显示行号（缺省为 true）。
- `context` 对应 `-C`，指定匹配行前后各显示多少行上下文，仅在 `content` 模式下生效（缺省为 0）。
- `hidden` 控制是否包含隐藏文件和目录（缺省为 false）。
- `respect_ignore` 控制是否遵循 `.gitignore` 和其他 ignore 规则（缺省为 true）。
- `limit` 限制整个搜索返回的匹配行数（`content` 模式，上下文不计入限额）或文件数（其余模式）（缺省 250，传 0 表示不限制）；只有存在被省略的匹配行或文件时，`truncated` 才为 `true`。`content` 和 `count` 的输出顺序取决于并行遍历顺序。
- `content` 中匹配行使用 `:` 分隔，上下文行使用 `-` 分隔。达到限额后仍保留已接受匹配的后置上下文，其中再次命中的行也只作为上下文展示。
- 遍历、读取或忽略规则解析失败会返回错误；`content` 模式的搜索缓冲区上限为 10 MiB，超长行或上下文超过缓冲区容量时也会返回错误。
- 工具会以工作目录为根递归查找文件；默认遵循 `.gitignore` 等忽略规则，并跳过隐藏文件/目录。
- 当需要进行多轮交叉的 glob + grep 搜索时，可以改用 `Agent` 工具委派给子智能体。
