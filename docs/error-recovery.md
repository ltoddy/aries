# 从错误中恢复

### 模型输出截断

例如 anthropic 的接口可以设置 max_tokens, 如果模型在 max_tokens 的数量内还没有回答完,输出就会停止.

- 对于本次会话调整更大的 max_tokens
- 保存截断内容, 再次发起一轮会话.

### 上下文超长

agent loop 内每轮工具调用/结果都会写入上下文, 一次任务连续访问大量文件、执行大量命令时上下文会突发性增长, 可能在有限轮数内超出模型窗口.

通过 rig 的 `AgentHook::on_completion_call` 做发送端过滤: 每轮 completion 前用 `TokenEstimator` 估算 history + prompt 的 token 数, 达到窗口的 80% (`ContextWindow::near_overflow_threshold`) 时, 将最老的工具调用/结果替换为占位符 (`micro_compact`, 保留最近 30 条), 再发送给模型. 只影响本轮发送, rig 内部消息与 transcript 完整保留, 零 LLM 调用, 不打断 agent 流程.

待做:
- 全量压缩 (`PromptTooLong`) 失败后的降级策略
- 模型侧 `context_length_exceeded` 报错的兜底 (`AriesError::is_context_exceeded` 补调用点)

### 模型提供商故障

例如遇到 429 限流, 529 供应商负载过高.

通过指数退避算法进行调用重试.
同时也可以考虑更换模型.
