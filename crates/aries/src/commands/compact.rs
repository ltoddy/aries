use aries_session::Session;
use rig::agent::PromptHook;
use rig::providers::{azure, openai};

use crate::theme::Theme;

pub const NAME: &str = "/compact";

pub async fn execute<P>(session: &mut Session<P>, theme: &Theme)
where
    P: PromptHook<openai::CompletionModel> + PromptHook<azure::CompletionModel> + 'static,
{
    match session.compact().await {
        Ok(()) => println!("{}", theme.green_text("Context compacted successfully.")),
        Err(err) => eprintln!("{}: {}", theme.red_text("Failed to compact context"), err),
    }
}
