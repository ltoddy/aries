use aries_session::Session;

use crate::theme::Theme;

pub async fn execute(session: &mut Session, theme: &Theme) {
    if session.compact().await {
        println!("{}", theme.green_text("对话压缩成功。"))
    } else {
        eprintln!("{}", theme.red_text("没有可压缩的内容。"))
    }
}
