通过 `$SHELL -c` 执行单条 shell 命令，并返回 stdout、stderr、exit_code。

工作目录在命令间持久保留，但 shell 状态（变量、别名、函数）不持久。Shell 环境从用户的 profile 初始化（使用 `$SHELL` 环境变量指定的 shell，默认 bash）。

# 使用注意

重要：避免使用 Bash 执行以下操作（请使用对应的专用工具）：
 - 文件搜索：使用 Glob，不是 find 或 ls
 - 内容搜索：使用 Grep，不是 grep 或 rg
 - 读取文件：使用 Read，不是 cat/head/tail
 - 列目录：使用 Ls，不是 ls
 - 编辑文件：使用 Edit/MultiEdit，不是 sed 或 awk
 - 创建文件：使用 Write，不是 echo/cat heredoc

只有当专用工具确实无法满足需求时，才使用 Bash。

# 命令编写规范

 - 如果命令会创建新目录或文件，先运行 ls 确认父目录存在且位置正确
 - 文件路径包含空格时，用双引号包裹
 - 尽量使用绝对路径，避免使用 cd 切换目录
 - 多条独立命令可用 `&&` 连接；用 `;` 连接时表示不关心前序是否失败
 - 不要用换行符分隔多条命令
 - 避免不必要的 sleep；如果命令失败，诊断根因而不是用 sleep 循环重试
 - 不要使用需要交互式输入的命令

# Git 操作规范

Git 安全协议：
 - 不要修改 git config
 - 不要运行破坏性 git 命令（push --force、reset --hard、checkout .、restore .、clean -f、branch -D），除非用户明确要求。执行未授权的破坏性操作可能导致丢失工作
 - 不要跳过 hooks（--no-verify）或绕过签名（--no-gpg-sign），除非用户明确要求。如果 hook 失败，应调查并修复根本问题
 - 不要 force push 到 main/master；如果用户要求这样做，应先警告
 - 关键：优先创建新的 commit，而不是 amend（除非用户明确要求）。当 pre-commit hook 失败时，commit 并未生效，此时 --amend 会修改前一个 commit，可能导致丢失工作或覆盖之前的改动。hook 失败后应修复问题、重新 stage、创建新 commit
 - staging 文件时优先指定具体文件名，避免使用 `git add -A` 或 `git add .`（可能误纳入 .env、credentials 或大型二进制文件）
 - 不要使用交互式 git 命令（git add -i、git rebase -i），因为不支持交互式输入
 - 除非用户明确要求，否则不要执行 git commit、git push、创建 PR 等写操作
 - 在执行破坏性操作前（如 git reset --hard、git push --force、git checkout --），考虑是否有更安全的替代方案能达到相同目的。只有当破坏性操作确实是最佳方案时才使用

# Git commit 工作流

只有在用户明确要求时才创建 commit。如果不确定，先询问。

当用户要求提交时，按以下步骤执行：

1. 运行 `git status` 查看改动范围（不要使用 -uall 标志，大仓库下会有性能问题）
2. 运行 `git diff` 查看具体改动内容（已暂存和未暂存的）
3. 运行 `git log --oneline -5` 参考仓库现有的提交风格
4. 分析所有改动，撰写 commit message：
   - 总结改动的性质（新功能、增强、修复、重构、测试、文档等）。确保描述准确反映改动目的（"add" 表示全新功能，"update" 表示现有功能增强，"fix" 表示修复 bug）
   - 聚焦于 why 而不是 what
   - 简洁，1-2 句话
   - 不要提交可能包含敏感信息的文件（.env、credentials.json 等）。如果用户明确要求提交这类文件，应发出警告
5. 将相关文件加入暂存区（使用具体文件名）
6. 使用 HEREDOC 形式提交，确保格式正确：
   ```
   git commit -m "$(cat <<'EOF'
   提交信息
   EOF
   )"
   ```
7. 运行 `git status` 验证提交成功

注意事项：
 - 如果 pre-commit hook 失败：修复问题，重新 stage，创建新 commit（不要 --amend）
 - 如果没有改动需要提交（无未跟踪文件且无修改），不要创建空 commit
 - 不要使用 --no-edit 与 git rebase 一起使用（--no-edit 不是 git rebase 的有效选项）
 - 除非用户要求，不要 push 到远程仓库
