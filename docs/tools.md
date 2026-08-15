# 内置工具 Tools

aries 内置 16 个工具，按模式分配。

## 基础工具（所有模式可用）

| 工具         | 说明                                                       |
|--------------|------------------------------------------------------------|
| `bash`       | 执行 shell 命令                                            |
| `read`       | 读取文件，支持行号输出、limit 分页、流式读取               |
| `glob`       | 文件模式匹配搜索                                           |
| `grep`       | 正则搜索文件内容                                           |
| `codesearch` | 通过 Exa 远程服务检索第三方库 / API / 框架的代码示例与文档 |
| `webfetch`   | 抓取网页（Firecrawl）                                      |
| `websearch`  | 网络搜索（Tavily）                                         |

## Build / General 模式额外可用

| 工具          | 说明                                                      |
|---------------|-----------------------------------------------------------|
| `write`       | 创建新文件或覆盖空文件，输出新增行数                    |
| `edit`        | 精确字符串替换编辑文件                                    |
| `multiedit`   | 对同一文件依次执行多次查找替换                            |
| `batch`       | 并发执行多个独立工具调用                                  |
| `lsp`         | 与 LSP 服务器交互（定义跳转、引用查找、悬停等）           |
| `question`    | 向用户提问（选择 / 多选 / 自由作答）                      |
| `skill`       | 加载并执行 skill                                          |
| `update_plan` | 推送 / 更新执行计划（支持 active_form、全部完成自动清空） |

Plan 模式额外可用 `question`；Explore 模式仅有基础工具。

## 工具优化

aries 对工具实现做了一系列优化：

### ToolContext 共享状态

写入类工具（`write` / `edit` / `multiedit`）共享同一个 `ToolContext`，包含：

- `SharedReadState`：记录每个文件被读取时的修改时间戳。
- `SharedFileCheckpoint`：写前备份文件内容（超过 1 MB 跳过，避免占用内存），支持回滚。
- LSP 客户端：可选，写入后用于通知 LSP 重建索引。

### 写前校验 guard_write

`edit` / `multiedit` 在写入前执行 `guard_write` 校验：

- 文件必须已被 `read` 工具读取过（否则报 `NotRead`）。
- 文件在读取后未被外部修改（否则报 `ModifiedSinceRead`）。

`write` 工具则通过拒绝覆盖非空文件来保护已有内容（只允许创建新文件或覆盖空文件）。

### 写后通知 LSP 重建索引

`write` / `edit` / `multiedit` 写入后会调用 LSP 的 `did_change` + `did_save`，让语言服务器及时重建索引，随后更新
`read_state`。

### 结构化 diff

`write` / `edit` / `multiedit` 复用共享的 `diff` 模块（`tools/diff.rs`，基于 `similar` 的 `TextDiff`），输出结构化 diff：

- `Hunk`：`old_start` / `old_lines` / `new_start` / `new_lines`，以及带 `+` / `-` 前缀的行内容。
- `additions` / `deletions`：新增 / 删除行数。

`write` 只创建新文件或覆盖空文件，输出附带 `additions`（新增行数）；`edit` / `multiedit` 输出 `WriteKind::Update`，附带 `original_content` 与 `structured_patch`。

### Read 流式读取

`read` 工具采用 `BufReader` 逐行流式读取，避免一次性加载大文件，并支持：

- `offset`（1-indexed 起始行）与 `limit`（读取行数）分页。
- 行号输出（`{:>6}→` 前缀）。
- 默认最多读取 2000 行。
- 空文件返回 `<system-reminder>` 提示而非空内容。

### UpdatePlan 增强

`update_plan` 工具支持：

- `PlanEntry` 包含 `content`（祈使句）+ `active_form`（进行时）+ `priority`（high / medium / low）+ `status`（pending /
  in_progress / completed）。
- 校验 `content` / `active_form` 非空。
- 所有条目均为 `completed` 时自动清空。

### Skill 工具增强

`skill` 工具支持：

- `allowed-tools`：技能声明其允许使用的工具集。
- 强制调用规则：当技能匹配用户请求时，必须先调用对应 skill。
