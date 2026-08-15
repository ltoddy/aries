# 长期记忆系统

aries 的长期记忆让 AI 助手能够跨会话记住用户信息、偏好与项目背景。

## 记忆类型

记忆通过 frontmatter 的 `type` 字段区分：

- `user`：用户的角色、目标、偏好和知识背景。
- `feedback`：用户对工作方式的指导。
- `project`：关于进行中的工作、目标或事件的信息。
- `reference`：指向外部系统中信息位置的指针。

每条记忆是一个带 frontmatter 的 markdown 文件，frontmatter 包含 `name`、`description`、`type` 三个字段。

## 架构

长期记忆由三个角色组成：

- **MemoryStore**：基于文件持久化，按项目目录隔离，负责扫描、读取记忆与 `MEMORY.md` 索引（manifest）。
- **MemoryRetriever**：在 prompt 前根据当前查询召回相关记忆（最多 5 条），注入为 `system-reminder`（用户不可见）。
- **MemoryAgent**：一个受限工具集（read / write / edit / glob / grep）的后台子代理，在 prompt 后分析对话并写入 /
  更新记忆，不阻塞主流程。

## 工作流程

1. 用户提交 prompt 前，`recall_context` 扫描记忆并召回相关内容。
2. 相关记忆以 `<system-reminder>` 形式注入历史。
3. 一轮对话结束后，后台 `MemoryAgent` 判断是否有值得跨会话保存的经验，并写盘或更新 `MEMORY.md` 索引。

## 存储位置

记忆文件存放在 `~/.local/share/aries/projects/<project-slug>/` 下，索引文件为 `MEMORY.md`。
