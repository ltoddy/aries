退出本会话通过 `EnterGitWorktree` 创建的活动 worktree，并恢复进入前的工作目录。用于用户要求离开、回到原目录或明确清理该 worktree；不管理用户手动创建的 worktree，也不要主动调用。

- `action` 必填：`keep` 退出并保留 worktree 目录和分支；用户只要求离开时使用它。`remove` 退出并删除 worktree 目录及对应分支，仅在用户明确要求删除、清理或放弃该工作时使用。
- `remove` 不检查未提交文件或新增提交，也不会自动保存、合并或备份这些工作；调用前须确认删除范围符合用户要求。
- 成功返回 `action`、`original_cwd`、原 worktree 路径、分支和提示。`keep` 返回的路径仍保留工作，`remove` 返回的路径只是被删除目标的记录。
- 退出后该会话不再跟踪此 worktree，不能再用本工具删除已 `keep` 的 worktree；再次调用会因没有活动 worktree 报错。`EnterGitWorktree` 可创建新的 worktree，不能据此恢复旧会话。
- 删除目录和分支是分步操作，错误不保证已完成的删除会回滚；失败后先检查目录和分支状态，再决定后续操作。
