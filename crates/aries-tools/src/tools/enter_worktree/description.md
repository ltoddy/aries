基于当前 Git `HEAD` 创建新的 worktree 和分支，并将当前工具会话切换到该目录，用于用户明确要求在 worktree 中隔离开展工作。仅要求创建或切换分支时使用 Git 命令；不要因一般开发任务主动调用本工具。

当前目录必须位于有可用 `HEAD` 的 Git 工作仓库中，且会话没有本工具创建的活动 worktree。新目录位于 Aries 配置的 worktree 根目录下，从已提交的 `HEAD` 开始，不复制原目录的未提交修改或未跟踪文件。

成功返回 `worktree_path`、`worktree_branch` 和切换提示，表示已创建并进入；后续相对文件路径和 shell 命令使用新目录，绝对路径仍指向原位置。离开时用 `ExitGitWorktree`，不要重复调用来查询状态。
