use colored::Colorize;
use rig::completion::Message;

pub const NAME: &str = "/save-history";

pub async fn execute(chat_history: &[Message]) {
    if let Ok(content) = serde_json::to_string_pretty(chat_history) {
        match tokio::fs::write("chat-history.json", content).await {
            Ok(_) => println!("{}", "Chat history saved to chat-history.json".green()),
            Err(err) => eprintln!("{}: {}", "Failed to save history".red(), err),
        }
    }
}
