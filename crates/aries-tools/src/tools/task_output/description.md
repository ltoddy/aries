读取后台任务的输出和状态。

后台任务由 Bash 工具的 `background` 参数或 Monitor 工具启动。使用 `task_id` 查询任务当前输出、错误输出、退出码和状态。

参数：
- `task_id`：后台任务 ID。
- `block`：是否等待任务结束后再返回，默认为 true。

如果只想查看当前输出而不等待完成，设置 `block` 为 false。