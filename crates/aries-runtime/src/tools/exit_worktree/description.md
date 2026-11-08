退出本会话通过 `EnterGitWorktree` 创建的活动 worktree，并恢复进入前的工作目录。用于用户要求离开、回到原目录或明确清理该 worktree；不管理手动创建的 worktree，也不要主动调用。

- 用户只要求离开时选 `keep`。`remove` 必须符合用户明确的删除、清理或放弃意图：它不检查未提交文件或新增提交，也不会自动保存、合并或备份这些工作。
- 成功返回 `action`、`original_cwd`、原 worktree 路径、分支和提示。`keep` 返回的路径仍保留工作，`remove` 返回的路径只是被删除目标的记录。
- 退出后会话不再跟踪此 worktree，不能再用本工具删除已 `keep` 的目录；再次调用会因没有活动 worktree 报错。`EnterGitWorktree` 可创建新的 worktree，不能据此恢复旧会话。
- 删除目录和分支是分步操作，失败不保证回滚已完成的删除；先检查目录和分支状态，再决定后续操作。
