use std::io::Write;

use aries_theme::Theme;
use rig::agent::{HookAction, PromptHook, ToolCallHookAction};
use rig::completion::{CompletionModel, CompletionResponse, GetTokenUsage, Message};

use crate::display::{display_token_usage, format_tool_call_args, format_tool_result_output};

#[derive(Debug, Clone)]
pub struct DisplayPromptHook {
    theme: Theme,
}

impl DisplayPromptHook {
    pub fn new(theme: Theme) -> Self {
        Self { theme }
    }
}

impl<M: CompletionModel> PromptHook<M> for DisplayPromptHook {
    async fn on_completion_response(
        &self,
        _prompt: &Message,
        response: &CompletionResponse<M::Response>,
    ) -> HookAction {
        display_token_usage(&response.usage, &self.theme);
        HookAction::cont()
    }

    async fn on_tool_call(
        &self,
        tool_name: &str,
        _tool_call_id: Option<String>,
        _internal_call_id: &str,
        args: &str,
    ) -> ToolCallHookAction {
        let formatted_tool = format_tool_call_args(tool_name, args, &self.theme);
        println!("\n{} {}", self.theme.cyan_text("•"), formatted_tool);
        ToolCallHookAction::cont()
    }

    async fn on_tool_result(
        &self,
        tool_name: &str,
        _tool_call_id: Option<String>,
        _internal_call_id: &str,
        _args: &str,
        result: &str,
    ) -> HookAction {
        let output_str = format_tool_result_output(tool_name, result);

        let max_lines = 7;
        let lines: Vec<&str> = output_str.lines().collect();

        if lines.len() > max_lines {
            for line in lines.iter().take(max_lines) {
                println!("  {}", self.theme.dimmed(line));
            }
            println!("  ... ({} more lines truncated)", lines.len() - max_lines);
        } else {
            for line in lines {
                println!("  {}", self.theme.dimmed(line));
            }
        }

        HookAction::cont()
    }

    async fn on_text_delta(&self, text_delta: &str, _aggregated_text: &str) -> HookAction {
        print!("{}", text_delta);
        let _ = std::io::stdout().flush();
        HookAction::cont()
    }

    async fn on_stream_completion_response_finish(
        &self,
        _prompt: &Message,
        response: &<M as CompletionModel>::StreamingResponse,
    ) -> HookAction {
        if let Some(usage) = response.token_usage() {
            display_token_usage(&usage, &self.theme);
        }
        HookAction::cont()
    }
}
