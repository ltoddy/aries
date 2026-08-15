# 上下文管理

## 历史与上下文分离

`chat-history`（完整历史）与 `context`（用于 prompt 的上下文）分开持久化。

## 上下文压缩

- `pre_compact`：prompt 前按预估 token 预判压缩。
- `post_compact`：prompt 后按实际 token 复核压缩。
- `micro_compact`：保留更多最近消息的轻量压缩。
- `AutoCompactBreaker`：压缩失败冷却机制，避免反复重试。

## Token 预估

通过 `TokenEstimator` 预估 token，`ContextWindow` 提供自动压缩阈值。
