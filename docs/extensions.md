# 扩展系统 Extensions

aries 通过 `AgentExtensions` 统一加载 5 类扩展，支持从当前目录与用户主目录加载：

| 扩展    | 说明                                                                             |
|---------|----------------------------------------------------------------------------------|
| Agent   | 自定义 agent 定义（name / description / tools / disallowed-tools / model）       |
| Command | 自定义 slash command（支持 `$1` / `$ARGUMENTS` 参数展开、allowed-tools）         |
| Hook    | 生命周期钩子（见 [hooks.md](hooks.md)）                                          |
| MCP     | Model Context Protocol 服务（stdio / sse / http 三种传输）                       |
| Skill   | 技能定义（name / description / allowed-tools / metadata，返回结构化 skill 内容） |

`--bare` 模式下不加载任何扩展。

## MCP

MCP 服务支持三种传输方式：

- `stdio`：本地进程通信
- `sse`：Server-Sent Events
- `http`：HTTP

## Skill

Skill 是带 frontmatter 的 markdown 文件，frontmatter 支持 `name`、`description`、`allowed-tools`、`metadata` 等字段。加载后会生成对应的
`skill` 工具，供主对话调用。
