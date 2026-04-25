use aries_core::jsonl;
use aries_theme::Theme;
use rig::completion::Message;

pub const NAME: &str = "/save-history";

pub async fn execute(history: &[Message], theme: &Theme) {
    match jsonl::write("chat-history.jsonl", history).await {
        Ok(_) => println!("{}", theme.green_text("Chat history saved to chat-history.jsonl")),
        Err(err) => eprintln!("{}: {}", theme.red_text("Failed to save history"), err),
    }
}
