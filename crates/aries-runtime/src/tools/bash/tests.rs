// This file contains tests generated with AI assistance.

use rig::tool::{Tool, ToolContext};
use tempfile::TempDir;

use crate::state::State;
use crate::tools::bash::{BashArgs, BashTool};

fn bash_args(command: &str) -> BashArgs {
    BashArgs { command: command.to_owned(), description: None, background: false }
}

fn state(cwd: impl Into<std::path::PathBuf>) -> State {
    let config = aries_init::ModelConfig::openai("test", "test", "test", "http://localhost");
    let model = crate::create_model(&config).expect("test model should initialize");
    let frontmatter = serde_json::from_value(serde_json::json!({
        "name": "test",
        "description": "test agent"
    }))
    .expect("test frontmatter should initialize");
    let cwd = cwd.into();
    let agent = aries_extension::agent::AgentDefinition::new(&cwd, frontmatter, "");
    let session_dir = cwd.join(".aries-test-session");
    State::new(model, agent, &cwd, &session_dir, "test", &session_dir)
}

async fn tool(cwd: impl Into<std::path::PathBuf>) -> BashTool {
    let ctx = crate::tools::context::ToolContext::new(
        {
            let (notifier, _) = aries_communicate::Notifier::channel();
            notifier
        },
        cwd,
    );
    let state = state(ctx.cwd());
    let gctx = aries_init::GlobalContext::new().await;
    BashTool::new(ctx, state, gctx)
}

#[tokio::test]
async fn test_bash_nonexistent_command() {
    let mut context = ToolContext::new();
    let tool = tool(std::env::temp_dir()).await;
    let result = tool
        .call(&mut context, bash_args("nonexistent_cmd_12345"))
        .await
        .expect("test operation should succeed");

    assert_ne!(result.exit_code, 0);
    assert!(!result.stderr.is_empty());
}

#[tokio::test]
async fn test_bash_args_title() {
    let args = bash_args("echo test");
    assert_eq!(args.title(), "Run shell command: echo test");

    let empty_args = bash_args("");
    assert_eq!(empty_args.title(), "Run a shell command");
}

#[tokio::test]
async fn test_description_excludes_git_section_outside_repo() {
    let dir = TempDir::new().expect("test operation should succeed");
    let tool = tool(dir.path()).await;

    assert!(!tool.description().contains("# 使用 git 提交改动"));
}

#[tokio::test]
async fn test_description_includes_git_section_inside_repo() {
    let dir = TempDir::new().expect("test operation should succeed");
    git2::Repository::init(dir.path()).expect("test operation should succeed");
    let tool = tool(dir.path()).await;

    assert!(tool.description().contains("# 使用 git 提交改动"));
}
