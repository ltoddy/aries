读取本会话后台 shell 或监控任务的状态与已保留输出，用于检查进度、诊断失败或取得最终结果；不会重新执行或停止任务，停止请用 `TaskStop`。

- 默认等待任务结束且没有超时参数；检查持续运行的监控任务时应设 `block=false`。
- 返回 `status`（`running` / `completed` / `failed` / `killed`）、`exit_code`、标准输出 `output` 和标准错误 `error`。`retrieval_status=not_ready` 表示仍在运行，`success` 表示已结束，不代表命令成功；须结合状态和退出码判断。
- 每次返回当前保留内容，不是读取后的增量，也不会消费输出。stdout / stderr 各最多保留 256 KiB，超限丢弃最早内容且没有截断标记；完整日志须由原命令另行保存。
- 任务记录只存在于当前会话，结束的任务仅保留最近 50 个；ID 不存在或记录已淘汰时返回错误。
