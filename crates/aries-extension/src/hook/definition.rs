use std::collections::HashMap;
use std::io;
use std::path::Path;

use regex_lite::Regex;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::info;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HooksDefinition {
    pub description: Option<String>,
    pub hooks: HooksSettings,
}

#[derive(Debug, Error)]
pub enum HooksFileParseError {
    #[error("failed to read hooks file: {0}")]
    Io(#[from] io::Error),
    #[error("failed to parse hooks file: {0}")]
    Json(#[from] serde_json::Error),
}

impl HooksDefinition {
    pub async fn parse(file_path: impl AsRef<Path>) -> Result<Self, HooksFileParseError> {
        let file_path = file_path.as_ref();
        info!("Parsing hooks file: {}", file_path.display());

        let content = tokio::fs::read_to_string(file_path).await?;
        Ok(serde_json::from_str::<Self>(&content)?)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(transparent)]
pub struct HooksSettings(pub HashMap<HookEvent, Vec<HookMatcher>>);

#[derive(Debug, Clone, Copy, Hash, Eq, PartialOrd, PartialEq, Deserialize, Serialize)]
pub enum HookEvent {
    /// Fires when a session starts.
    SessionStart,

    /// Fires during setup before normal request handling begins.
    Setup,

    /// Fires when the user submits a prompt.
    UserPromptSubmit,

    /// Fires after the user prompt is expanded with additional context.
    UserPromptExpansion,

    /// Fires before a tool is executed.
    ///
    /// Common uses include validating commands, blocking unsafe edits, and
    /// enforcing project rules before the action runs.
    PreToolUse,

    /// Fires when the agent is about to request permission.
    PermissionRequest,

    /// Fires after a permission request is denied.
    PermissionDenied,

    /// Fires after a single tool call completes successfully.
    ///
    /// Common uses include formatting edited files, refreshing generated state,
    /// or recording audit logs.
    PostToolUse,

    /// Fires after a single tool call fails.
    PostToolUseFailure,

    /// Fires after a batch of tool calls completes.
    PostToolBatch,

    /// Fires when the agent is waiting for input or permission.
    ///
    /// Commonly used to send a desktop or external notification so you do not
    /// need to watch the terminal.
    Notification,

    /// Fires when a subagent starts.
    SubagentStart,

    /// Fires when a subagent stops.
    SubagentStop,

    /// Fires when a background or asynchronous task is created.
    TaskCreated,

    /// Fires when a background or asynchronous task completes.
    TaskCompleted,

    /// Fires when the current turn stops normally.
    Stop,

    /// Fires when the current turn stops because of a failure.
    StopFailure,

    /// Fires when a teammate or collaborating agent becomes idle.
    TeammateIdle,

    /// Fires after instructions are loaded.
    InstructionsLoaded,

    /// Fires when configuration changes.
    ///
    /// `ConfigChange` can be used to track when settings or skills files change
    /// during a session.
    ConfigChange,

    /// Fires when the current working directory changes.
    ///
    /// Useful for reloading environment-dependent state when moving between
    /// directories.
    CwdChanged,

    /// Fires when files change.
    FileChanged,

    /// Fires when a git worktree is created.
    WorktreeCreate,

    /// Fires when a git worktree is removed.
    WorktreeRemove,

    /// Fires before compaction starts.
    PreCompact,

    /// Fires after compaction completes.
    PostCompact,

    /// Fires when the agent asks the user for additional input or clarification.
    Elicitation,

    /// Fires after the user responds to an elicitation.
    ElicitationResult,

    /// Fires when a session ends.
    SessionEnd,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShellType {
    #[default]
    Bash,
    Powershell,
    Sh,
    Zsh,
}

impl ShellType {
    pub fn invocation(&self) -> (&'static str, &'static str) {
        match self {
            ShellType::Bash => ("bash", "-c"),
            ShellType::Sh => ("sh", "-c"),
            ShellType::Zsh => ("zsh", "-c"),
            ShellType::Powershell => ("powershell", "-Command"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum HookCommand {
    Command(BashCommandHook),
    Prompt(PromptHook),
    // Agent(AgentHook),
    Http(HttpHook),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BashCommandHook {
    /// 要执行的 shell 命令
    pub command: String,

    /// 权限规则语法过滤，例如 "Bash(git *)"
    #[serde(default, rename = "if", skip_serializing_if = "Option::is_none")]
    pub if_condition: Option<String>,

    /// shell 解释器，默认 bash
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell: Option<ShellType>,

    /// 单个命令超时时间（秒）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<f64>,

    /// 自定义状态条提示文案
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_message: Option<String>,

    /// true 时执行一次后被移除
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub once: Option<bool>,

    /// true 时后台运行，不阻塞
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#async: Option<bool>,

    /// true 时后台运行，并在 exit code 2 时唤醒模型；隐含 async
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub async_rewake: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptHook {
    /// 用 LLM 评估的 Prompt，可使用 $ARGUMENTS 占位符
    pub prompt: String,

    #[serde(default, rename = "if", skip_serializing_if = "Option::is_none")]
    pub if_condition: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<f64>,

    /// 模型 ID，例如 "gpt-4o"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_message: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub once: Option<bool>,
}

// 目前处于试验阶段, 先不添加
// #[derive(Debug, Clone, Serialize, Deserialize)]
// pub struct AgentHook {
//     /// 描述要验证什么的 Prompt
//     pub prompt: String,
//
//     #[serde(default, rename = "if", skip_serializing_if = "Option::is_none")]
//     pub if_condition: Option<String>,
//
//     /// 默认 60 秒
//     #[serde(default, skip_serializing_if = "Option::is_none")]
//     pub timeout: Option<f64>,
//
//     #[serde(default, skip_serializing_if = "Option::is_none")]
//     pub model: Option<String>,
//
//     #[serde(default, skip_serializing_if = "Option::is_none")]
//     pub status_message: Option<String>,
//
//     #[serde(default, skip_serializing_if = "Option::is_none")]
//     pub once: Option<bool>,
// }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpHook {
    /// POST 目标 URL
    pub url: String,

    #[serde(default, rename = "if", skip_serializing_if = "Option::is_none")]
    pub if_condition: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<f64>,

    /// 额外 header，值支持 $VAR / ${VAR} 插值
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<HashMap<String, String>>,

    /// 允许插值的环境变量白名单
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_env_vars: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_message: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub once: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookMatcher {
    /// 匹配模式，例如工具名 "Write"、"Bash"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matcher: Option<String>,

    /// 匹配命中时执行的 hooks
    pub hooks: Vec<HookCommand>,
}

#[derive(Debug, Error)]
pub enum HookMatcherError {
    #[error("invalid hook matcher regex {pattern:?}: {source}")]
    InvalidRegex {
        pattern: String,
        #[source]
        source: regex_lite::Error,
    },
}

impl HookMatcher {
    pub fn matches(&self, tool_name: impl Into<String>) -> Result<bool, HookMatcherError> {
        let tool_name = tool_name.into();

        let pattern = match self.matcher.as_deref() {
            None => return Ok(true),
            Some(p) => p.trim(),
        };

        if pattern.is_empty() || pattern == "*" {
            return Ok(true);
        }

        // 完整匹配：用 `\A(?:...)\z` 显式锚定，避免 pattern 内部的 `|`
        // 改变锚点优先级（例如 "Edit|Write" 应等价于 "\A(?:Edit|Write)\z"）。
        let anchored = format!(r"\A(?:{})\z", pattern);
        let re = Regex::new(&anchored).map_err(|err| HookMatcherError::InvalidRegex {
            pattern: pattern.to_string(),
            source: err,
        })?;
        Ok(re.is_match(&tool_name))
    }
}
