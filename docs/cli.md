# CLI 命令

aries 提供以下子命令：

| 命令                                              | 说明                                       |
|---------------------------------------------------|--------------------------------------------|
| `aries setup`                                     | 初始化配置                                 |
| `aries model add / rm / list / default / current` | 管理模型配置（增删查、切换默认、查看当前） |
| `aries prompt`                                    | 一次性发送 prompt                          |
| `aries session list / resume / prune`             | 管理会话（列出、恢复、清理）               |
| `aries acp`                                       | 启动 Agent Client Protocol (ACP) 服务      |
| `aries agent list`                                | 列出自定义 agent                           |
| `aries command list`                              | 列出 slash command                         |
| `aries mcp list`                                  | 列出 MCP 服务                              |
| `aries skill list`                                | 列出 skill                                 |
| `aries stats tool / bash`                         | 可视化统计工具调用次数与 bash 命令使用频率 |
| `aries gc`                                        | 清理过期数据库记录                         |
| `aries exec`                                      | 执行 shell 命令                            |

全局 `--bare` 标志：以 bare 模式快速启动 Agent，不加载任何扩展。

首次使用执行 `aries setup`；后续通过 `aries model add` / `aries model rm` 增删模型，`aries model default` 切换默认模型，
`aries model list` 查看所有模型。
