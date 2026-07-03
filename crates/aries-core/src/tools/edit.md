在已存在的文件中执行精确字符串替换。

- `file_path` 必须指向一个已存在的文件，否则失败。
- `old_text` 必须与文件中的现有内容完全匹配，包括空白和缩进。
- `old_text` 与 `new_text` 不能完全相同，否则失败。
- 当 `old_text` 在文件中不存在时，调用失败。
- 当 `old_text` 在文件中出现多次且未设置 `replace_all: true` 时，调用失败；请扩展 `old_text` 的上下文使其唯一，或显式启用 `replace_all`。
