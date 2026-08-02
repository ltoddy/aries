#[cfg(test)]
mod tests;

use std::path::Path;

use itertools::Itertools;
use rig_agent::client::AgentClientExt;
use rig_agent::completion::{CompletionModel, Prompt};
use rig_agent::tool::server::ToolServer;
use tracing::warn;

pub const PREAMBLE: &str = include_str!("preamble.md");

pub struct MemoryAgent<M>
where
    M: CompletionModel + 'static,
{
    inner: rig_agent::agent::Agent<M>,
}

impl<M> MemoryAgent<M>
where
    M: CompletionModel + 'static,
{
    const DEFAULT_MAX_TURNS: usize = 50;

    pub async fn new<C>(c: C, model: impl Into<String>, memory_dir: impl AsRef<Path>) -> Self
    where
        C: AgentClientExt<CompletionModel = M> + 'static,
    {
        let tool_names = [
            aries_tools::read::NAME,
            aries_tools::write::NAME,
            aries_tools::edit::NAME,
            aries_tools::glob::NAME,
            aries_tools::grep::NAME,
        ];
        let toolset = aries_tools::create_tools_from_tool_names(&tool_names, memory_dir, None, &[]);
        let tool_server_handle = ToolServer::new().run();
        tool_server_handle.append_toolset(toolset).await;

        let inner = c
            .agent(model)
            .name("memory-agent")
            .description("分析对话并将值得跨会话持久化的信息写入或更新到记忆系统的子代理")
            .preamble(PREAMBLE)
            .tool_server_handle(tool_server_handle)
            .default_max_turns(Self::DEFAULT_MAX_TURNS)
            .build();

        Self { inner }
    }

    pub async fn run(
        &self,
        manifest: Option<String>,
        user: impl Into<String>,
        assistant: impl Into<String>,
    ) {
        let user = user.into();
        let assistant = assistant.into();

        let manifest = match manifest {
            Some(content) => truncate_manifest(&content),
            None => "（当前没有任何记忆文件）".to_owned(),
        };

        let prompt = [
            "<existing-memories>",
            &manifest,
            "</existing-memories>",
            "\n",
            "<conversation>",
            "<user>",
            &user,
            "</user>",
            "<assistant>",
            &assistant,
            "</assistant>",
            "</conversation>",
        ]
        .join("\n");

        if let Err(err) = self.inner.prompt(prompt).await {
            warn!(err = %err, "memory-agent failed, memories not updated this turn");
        }
    }
}

pub const MAX_MANIFEST_LINES: usize = 200;
pub const MAX_MANIFEST_BYTES: usize = 25_000;

pub fn truncate_manifest(content: &str) -> String {
    let mut truncated = content;
    let mut was_truncated = false;

    if truncated.len() > MAX_MANIFEST_BYTES {
        // 在字符边界处安全截断
        let mut end = MAX_MANIFEST_BYTES;
        while end > 0 && !truncated.is_char_boundary(end) {
            end -= 1;
        }
        truncated = &truncated[..end];
        was_truncated = true;
    }

    if truncated.lines().count() > MAX_MANIFEST_LINES {
        was_truncated = true;
    }

    let mut out = truncated.lines().take(MAX_MANIFEST_LINES).join("\n");
    if was_truncated {
        out.push_str("\n\n（MEMORY.md 索引过长，已截断。请考虑整理合并记忆。）");
    }
    out
}
