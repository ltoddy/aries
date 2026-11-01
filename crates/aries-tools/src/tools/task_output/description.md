读取本会话后台 shell 或监控任务的状态与已保留输出，用于检查进度、诊断失败或取得最终结果；不会重新执行或停止任务，停止请用 `TaskStop`。

- `task_id` 必须来自本会话 `Bash(background=true)` 或 `Monitor` 的返回值，也可取自其任务通知；不能使用进程 PID 或 `Agent` 返回的会话 ID。
- `block` 默认 `true`，等待任务结束后返回，没有超时参数；持续运行的监控任务应设为 `false`，立即取得当前快照。
- 返回 `status`（`running` / `completed` / `failed` / `killed`）、`exit_code`、标准输出 `output` 和标准错误 `error`。`retrieval_status=not_ready` 表示仍在运行，`success` 表示已结束，不代表命令成功；须结合状态和退出码判断。
- 每次返回当前保留内容，不是从上次读取后的增量，也不会消费输出。stdout / stderr 各最多保留 256 KiB，超限会丢弃最早的内容且没有截断标记；完整日志须由原命令另行保存到文件。
- 任务记录只存在于当前会话，结束的任务仅保留最近 50 个；ID 不存在或记录已淘汰时返回错误。
