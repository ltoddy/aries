通过 `$SHELL -c` 执行单条 shell 命令，并返回 stdout、stderr、exit_code。

工作目录在命令间持久保留，但 shell 状态（变量、别名、函数）不持久。Shell 环境从用户的 profile 初始化（使用 `$SHELL` 环境变量指定的 shell，默认 bash 或 zsh）。

stdout/stderr 超过 30000 字符时会被截断，并在末尾提示被截断的行数。可选参数 `description` 用于简短描述命令用途（5-10 个词）。不要使用需要交互式输入的命令。

重要：避免使用本工具执行 `find`、`grep`、`cat`、`head`、`tail`、`sed`、`awk`、`echo` 等命令，除非用户明确要求，或你已确认专用工具无法完成任务。请优先使用对应的专用工具，这会带来更好的体验：

 - 文件搜索：使用 Glob（不是 find 或 ls）
 - 内容搜索：使用 Grep（不是 grep 或 rg）
 - 读取文件：使用 Read（不是 cat/head/tail）
 - 编辑文件：使用 Edit（不是 sed/awk）
 - 创建文件：使用 Write（不是 echo >/cat <<EOF）
 - 输出内容：直接输出文本（不是 echo/printf）

虽然 Bash 工具也能做类似的事，但优先使用内置专用工具会带来更好的体验，也更便于审查工具调用与授予权限。

# 使用说明

 - 如果命令会创建新目录或文件，先用本工具运行 `ls` 确认父目录存在且位置正确
 - 命令中包含空格的文件路径务必用双引号包裹（例如 `cd "path with spaces/file.txt"`）
 - 尽量在整个会话中使用绝对路径、避免使用 `cd`，以保持当前工作目录不变。仅当用户明确要求时才使用 `cd`
 - 执行多条命令时：
   - 如果命令相互独立、可并行，就在同一条消息里发起多个 Bash 工具调用。例如需要运行 "git status" 和 "git diff" 时，在一条消息里并行发起两个 Bash 调用
   - 如果命令相互依赖、必须顺序执行，用单个 Bash 调用并以 `&&` 连接
   - 仅当需要顺序执行但不关心前序命令是否失败时才用 `;`
   - 不要用换行符分隔多条命令（引号内的换行是允许的）
 - 执行 git 命令时：
   - 优先创建新 commit，而不是 amend 已有 commit
   - 执行破坏性操作前（如 git reset --hard、git push --force、git checkout --），先考虑是否有更安全的替代方案能达到相同目的。只有当破坏性操作确实是最佳方案时才使用
   - 除非用户明确要求，不要跳过 hooks（--no-verify）或绕过签名（--no-gpg-sign、-c commit.gpgsign=false）。如果 hook 失败，应调查并修复根本问题
