use std::io::{self, BufWriter};
use std::path::Path;

use ferris_says::say;

const TERMINAL_WIDTH: usize = 80;

pub async fn welcome(
    provider: impl AsRef<str>,
    model: impl AsRef<str>,
    session_id: impl AsRef<str>,
    user: impl AsRef<str>,
    cwd: impl AsRef<Path>,
) {
    let provider = provider.as_ref();
    let model = model.as_ref();
    let session_id = session_id.as_ref();
    let user = user.as_ref();
    let cwd = cwd.as_ref();
    let name = env!("CARGO_BIN_NAME");
    let version = env!("CARGO_PKG_VERSION");

    let greeting =
        if user.is_empty() { "Welcome!".to_string() } else { format!("Welcome, {user}!") };
    let info = [
        format!("{name} v{version}"),
        format!("{model} · {provider}"),
        format!("session: {session_id}"),
        format!("Work at: {}", cwd.display()),
    ]
    .join("\n");

    let width = TERMINAL_WIDTH.saturating_sub(12);

    println!("{greeting}");

    let mut stdout = BufWriter::new(io::stdout().lock());
    let _ = say(&info, width, &mut stdout);
    println!();
}
