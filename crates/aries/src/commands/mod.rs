use aries_context::GlobalContext;
use aries_session::Session;
use rig::agent::PromptHook;
use rig::providers::{azure, openai};

use crate::theme::Theme;

pub mod clear_history;
pub mod compact;
pub mod completer;
pub mod exit;
pub mod save_history;
pub mod setup;
pub mod shell;

pub async fn execute<P>(input: &str, theme: &Theme, gctx: &GlobalContext, session: &mut Session<P>)
where
    P: PromptHook<openai::CompletionModel> + PromptHook<azure::CompletionModel> + 'static,
{
    if let Some(command) = input.strip_prefix(shell::NAME) {
        return shell::execute(command, theme).await;
    }

    match input {
        exit::NAME => exit::exit(),
        save_history::NAME => {
            save_history::execute(session.history(), theme).await;
        },
        clear_history::NAME => {
            session.clear_history();
            println!("{}", theme.green_text("Chat history cleared."));
        },
        compact::NAME => {
            compact::execute(session, theme).await;
        },
        setup::NAME => {
            if let Err(e) = setup::execute(theme, &gctx.config_dir).await {
                eprintln!("Error: {}", e);
            }
        },
        _ => (),
    }
}
