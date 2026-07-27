use aries_session::Session;
use colored::Colorize;

pub async fn execute(session: &mut Session) {
    if session.compact().await {
        println!("{}", "对话压缩成功。".green())
    } else {
        eprintln!("{}", "没有可压缩的内容。".red())
    }
}
