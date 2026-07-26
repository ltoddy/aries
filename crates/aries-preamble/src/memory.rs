use std::path::Path;

pub fn section(mem_dir: impl AsRef<Path>) -> String {
    let mem_dir = mem_dir.as_ref();

    let mut preamble = format!(
        r#"<memory-system>

你拥有一个基于文件的持久化记忆系统，位于 `{}`。该目录已存在，可直接使用 Write 工具写入。

你应当随时间逐步构建这个记忆系统，使未来的对话能够完整了解用户是谁、用户期望的协作方式、哪些行为需要避免或保持，以及用户交给你的工作背后的上下文。

如果用户明确要求你记住某件事，立即保存。如果用户要求你忘记某件事，找到并删除对应条目。

## 记忆类型

- **user**: 用户的角色、目标、偏好和知识背景。
- **feedback**: 用户对工作方式的指导——包括应避免的做法和应保持的做法。
- **project**: 关于进行中的工作、目标或事件的信息，且无法从代码或 git 历史中推导出来。
- **reference**: 指向外部系统中信息位置的指针。

## 不应保存的内容

- 代码模式、架构、文件路径（可通过读代码获得）
- Git 历史或最近变更（使用 `git log`）
- 调试方案（修复已在代码中）
- 临时性任务细节

## 如何保存记忆

将每条记忆写入独立文件，使用以下 frontmatter 格式：

```markdown
---
name: {{记忆名称}}
description: {{一句话描述}}
type: {{user, feedback, project, reference}}
---

{{记忆内容}}
```

然后在 `MEMORY.md` 中添加索引条目：`- [标题](文件名.md) — 一句话摘要`
</memory-system>"#,
        mem_dir.display()
    );

    let file_path = mem_dir.join("MEMORY.md");
    match std::fs::read_to_string(file_path) {
        Ok(content) => {
            preamble.push_str(&format!("\n## MEMORY.md\n\n{}", content.trim()));
        },
        Err(_) => {
            preamble.push_str("\n## MEMORY.md\n\n当前 MEMORY.md 为空。\n");
        },
    }

    preamble
}
