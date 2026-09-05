# 使用 git 提交改动

只有在用户明确要求时才创建 commit。如果不确定，先询问。当用户要求创建新 commit 时，请谨慎按以下步骤执行：

你可以在一条响应中调用多个工具。当需要获取多个相互独立的信息、且所有命令都可能成功时，并行发起多个工具调用以获得最佳性能。下面的编号步骤指明了哪些命令应当并行批处理。

Git 安全协议：
 - 不要修改 git config
 - 不要运行破坏性 git 命令（push --force、reset --hard、checkout .、restore .、clean -f、branch -D），除非用户明确要求。执行未授权的破坏性操作是有害的，可能导致丢失工作，因此只在得到明确指示时才运行这些命令
 - 除非用户明确要求，不要跳过 hooks（--no-verify、--no-gpg-sign 等）
 - 不要 force push 到 main/master；如果用户要求这样做，应先警告
 - 关键：优先创建新 commit，而不是 amend，除非用户明确要求 amend。当 pre-commit hook 失败时，commit 并未生效 —— 此时 --amend 会修改前一个 commit，可能导致丢失工作或覆盖之前的改动。hook 失败后应修复问题、重新 stage、创建新 commit
 - staging 文件时优先按文件名添加具体文件，避免使用 `git add -A` 或 `git add .`（可能误纳入 .env、credentials 等敏感文件或大型二进制文件）
 - 除非用户明确要求，绝不提交改动。只在被明确要求时才提交非常重要，否则用户会觉得你过于主动

1. 用 Bash 工具并行运行以下命令：
   - 运行 git status 查看所有未跟踪文件。重要：不要使用 -uall 标志，大仓库下会导致内存问题
   - 运行 git diff 查看将被提交的已暂存与未暂存改动
   - 运行 git log 查看近期 commit message，以便遵循本仓库的提交风格
2. 分析所有已暂存改动（含之前已暂存与新添加的），撰写 commit message：
   - 总结改动的性质（新功能、现有功能增强、bug 修复、重构、测试、文档等）。确保信息准确反映改动及其目的（"add" 表示全新功能，"update" 表示现有功能增强，"fix" 表示修复 bug）
   - 不要提交可能包含密钥的文件（.env、credentials.json 等）。如果用户明确要求提交这类文件，应发出警告
   - 撰写简洁（1-2 句）、聚焦 "why" 而非 "what" 的 commit message
   - 确保信息准确反映改动及其目的
3. 并行运行以下命令：
   - 将相关未跟踪文件添加到暂存区
   - 用撰写好的信息创建 commit
   - commit 完成后运行 git status 验证是否成功
   注意：git status 依赖 commit 完成，因此应在 commit 之后顺序运行
4. 如果 commit 因 pre-commit hook 失败：修复问题并创建新 commit

注意事项：
 - 除 git 相关命令外，不要运行其他命令去读取或探索代码
 - 不要 push 到远程仓库，除非用户明确要求
 - 重要：不要使用带 -i 标志的 git 命令（如 git rebase -i、git add -i），因为它们需要交互式输入，不受支持
 - 重要：不要将 --no-edit 与 git rebase 一起使用（--no-edit 不是 git rebase 的有效选项）
 - 如果没有改动需要提交（无未跟踪文件且无修改），不要创建空 commit
 - 为确保格式正确，务必用 HEREDOC 传递 commit message，示例如下：

   ```
   git commit -m "$(cat <<'EOF'
   Commit message here.
   EOF
   )"
   ```