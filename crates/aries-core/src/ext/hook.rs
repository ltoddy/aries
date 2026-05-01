/// see more: https://code.claude.com/docs/en/hooks-guide
use serde::{Deserialize, Serialize};

/// Hook events fire at specific points in the Claude Code lifecycle.
///
/// Hooks provide deterministic control over behavior, ensuring certain actions
/// always happen rather than relying on the model to choose them. These events
/// can be used to enforce project rules, automate repetitive tasks, send
/// notifications, inject context, and integrate with external tooling.
///
/// Naming convention:
/// - `Pre*`: before an action executes
/// - `Post*`: after an action completes
/// - `*Failure`: after an action fails
/// - `*Start` / `*Stop` / `*Completed`: lifecycle boundaries for longer-running
///   work
#[derive(Debug, Deserialize, Serialize)]
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

    /// Fires when Claude is about to request permission.
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

    /// Fires when Claude is waiting for input or permission.
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
    /// In Claude Code, `ConfigChange` can be used to track when settings or
    /// skills files change during a session.
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

    /// Fires when Claude asks the user for additional input or clarification.
    Elicitation,

    /// Fires after the user responds to an elicitation.
    ElicitationResult,

    /// Fires when a session ends.
    SessionEnd,
}
