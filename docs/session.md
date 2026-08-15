# 会话管理

- 会话支持创建、恢复（`resume`）、列表、清理（`prune`）。
- **内置命令**：`/exit`、`/shell`、`/compact`、`/system-prompt`。
- **slash command**：支持内置与自定义 slash command。
- 会话可取消（`cancel`）、切换模型（`set_model`）与模式（`set_mode`）。

## 会话生命周期

- 启动时触发 `SessionStart` hook，结束时触发 `SessionEnd` hook。
- 会话目录位于 `~/.local/share/aries/session-<id>/`，包含 transcript 与配置。
