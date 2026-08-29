在后台执行 shell 命令，用于持续监控日志或长时间运行的进程。

Monitor 会立即返回后台任务 ID，命令继续在后台运行。你可以使用 TaskOutput 查看当前输出，使用 TaskStop 停止任务。

参数：
- `command`：要执行的 shell 命令。
- `description`：简短描述该监控任务。