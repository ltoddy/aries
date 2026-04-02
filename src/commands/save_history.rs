use rig::completion::Message;

use crate::theme::Theme;

pub const NAME: &str = "/save-history";

pub async fn execute(chat_history: &[Message], theme: &Theme) {
    if let Ok(content) = serde_json::to_string_pretty(chat_history) {
        match tokio::fs::write("chat-history.json", content).await {
            Ok(_) => println!("{}", theme.green_text("Chat history saved to chat-history.json")),
            Err(err) => eprintln!("{}: {}", theme.red_text("Failed to save history"), err),
        }
    }
}
