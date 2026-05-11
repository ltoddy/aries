use aries_session::Session;

use crate::theme::Theme;

pub async fn execute(session: &mut Session, theme: &Theme) {
    if session.compact().await {
        println!("{}", theme.green_text("Conversation compacted successfully."))
    } else {
        eprintln!("{}", theme.red_text("There is nothing to compact."))
    }
}
