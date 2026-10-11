创建一个隔离的 git worktree，并将当前工具会话切换到该 worktree。

使用前提：
- 仅当用户明确提到 `worktree` 时使用，例如“创建 worktree”、“使用 worktree”、“在 worktree 里做”等。
- 当前目录必须位于 git 仓库中。
- 当前会话不能已经处于本工具创建的 worktree session 中。

不要使用的场景：
- 用户只是要求创建分支、切换分支或在另一个分支上工作：请使用 git 命令。
- 用户只是要求修 bug 或开发功能：除非用户明确提到 `worktree`，否则使用普通 git 工作流。
- 不要主动使用此工具，必须等用户明确提到 `worktree`。

行为：
- 基于当前 `HEAD` 创建一个新分支。
- 将当前工具会话的工作目录切换到新 worktree；后续相对路径和 shell 命令都会在该 worktree 内执行。
- 如需中途离开 worktree，请使用 `ExitGitWorktree`。

参数：
- `name`：可选的 worktree 名称。不传时会自动生成随机名称。
