# Hook 系统

Aries 实现了完整的 Hook 系统，在 Agent 生命周期的各个节点触发钩子。

## 事件

支持约 30 种生命周期事件，包括：

- 会话：`SessionStart`、`SessionEnd`
- 用户交互：`UserPromptSubmit`、`UserPromptExpansion`、`Elicitation`、`ElicitationResult`
- 工具：`PreToolUse`、`PostToolUse`、`PostToolUseFailure`、`PostToolBatch`、`PermissionRequest`、`PermissionDenied`
- 子代理：`SubagentStart`、`SubagentStop`
- 后台任务：`TaskCreated`、`TaskCompleted`
- 回合：`Stop`、`StopFailure`
- 压缩：`PreCompact`、`PostCompact`
- 其他：`Setup`、`Notification`、`TeammateIdle`、`InstructionsLoaded`、`ConfigChange`、`CwdChanged`、`FileChanged`、
  `WorktreeCreate`、`WorktreeRemove`

## Hook 类型

4 种 hook 类型：

- **Command**：执行 shell 命令，支持 `if` 条件、超时、once、async、async_rewake 等。
- **Prompt**：用 LLM 评估的 prompt。
- **Agent**：用 agent 验证的 prompt。
- **Http**：POST 到指定 URL，支持 header 插值与环境变量白名单。

## 匹配

Hook 通过 `matcher` 字段按工具名匹配（如 `Write`、`Bash`），支持正则与 `*` 通配。
