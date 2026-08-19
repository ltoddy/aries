# 上下文管理

## 历史与上下文分离

`chat-history`（完整历史）与 `context`（用于 prompt 的上下文）分开持久化。

## 上下文压缩

上下文压缩分为历史级压缩与发送前轻量压缩两层。

### 1. 历史级压缩：`pre_compact` / `post_compact`

- `pre_compact` 在 prompt 发起前执行：先对 `chat_context.history_mut()` 做一次 `micro_compact(KEEP_RECENT)`，再估算 `history + prompt` 的 token；达到 `ContextWindow::auto_compact_threshold()` 后触发完整 `compact()`。
- `post_compact` 在拿到模型返回的 `usage.total_tokens` 后执行；实际 token 超过 `auto_compact_threshold()` 时触发完整 `compact()`。
- 完整压缩由 `ContextCompactor` 协调，串联压缩 agent、hooks 与 `AutoCompactBreaker` 冷却机制，并直接改写 `chat_context`。

### 2. 发送前轻量压缩：`SessionPromptHook::on_completion_call`

- 这一层在每轮 completion 发送前对本轮请求做发送端过滤。
- 先用 `TokenEstimator` 估算 `history + prompt` 的 token 数；未超过阈值且没有待注入的 instructions 时，直接继续。
- 当上下文达到 `stuffed` 状态（超过 `ContextWindow::sixty_percent_threshold()`）后，进入分级 `micro_compact`：
  - > 80%：保留最近 10 条消息
  - > 75%：保留最近 15 条消息
  - > 70%：保留最近 20 条消息
  - > 65%：保留最近 25 条消息
  - > 60%：保留最近 30 条消息
- 这一层的 `micro_compact` 只作用于本轮发送给模型的 `history patch`，不直接改写完整 transcript。
- 待注入的 instructions 也通过 patch 后的 history 一并发送。

## Token 预估

`TokenEstimator` 用于估算上下文 token，`ContextWindow` 统一提供压缩阈值。
