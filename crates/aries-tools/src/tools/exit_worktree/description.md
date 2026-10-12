退出由 `EnterGitWorktree` 创建的 worktree session，并将当前工具会话恢复到进入 worktree 前的工作目录。

作用范围：
- 只会操作当前会话中由 `EnterGitWorktree` 创建的 worktree。
- 不会处理用户手动通过 `git worktree add` 创建的 worktree。

使用时机：
- 用户明确要求“退出 worktree”、“离开 worktree”、“回到原目录”、“结束 worktree session”等。
- 如果用户只是想离开 worktree，使用 `action: "keep"`。
- 如果用户明确要求删除、移除、清理 worktree，或表达当前方案不要了、放弃这条路线，使用 `action: "remove"`。
- 不要主动调用此工具，必须等用户提出要求。

参数：
- `action`：必填，取值为 `"keep"` 或 `"remove"`。
  - `"keep"`：退出 session，保留 worktree 目录和分支。
  - `"remove"`：退出 session，并删除 worktree 目录和对应分支；不会检查是否存在未提交文件或新增提交。

行为：
- 将当前工具会话的工作目录恢复到调用 `EnterGitWorktree` 之前的位置。
- 退出后，可以再次调用 `EnterGitWorktree` 创建新的 worktree。
