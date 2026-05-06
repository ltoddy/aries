use aries_session::Session;

use crate::theme::Theme;

pub async fn execute(session: &mut Session, theme: &Theme) {
    match session.compact().await {
        Ok(()) => println!("{}", theme.green_text("Context compacted successfully.")),
        Err(err) => eprintln!("{}: {}", theme.red_text("Failed to compact context"), err),
    }
}
