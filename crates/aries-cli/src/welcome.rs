use std::io::{self, BufWriter};
use std::path::Path;

use aries_init::GlobalContext;
use ferris_says::say;
use terminal_size::{Width, terminal_size};

pub fn welcome(
    provider: impl Into<String>,
    model: impl Into<String>,
    session_id: impl Into<String>,
    context: &GlobalContext,
    cwd: impl AsRef<Path>,
) {
    let provider = provider.into();
    let model = model.into();
    let session_id = session_id.into();
    let cwd = cwd.as_ref();

    let name = env!("CARGO_BIN_NAME");
    let version = env!("CARGO_PKG_VERSION");

    let greeting = if context.user.is_empty() {
        "Welcome!".to_string()
    } else {
        format!("Welcome, {}!", context.user)
    };
    let info = [
        format!("{name} v{version}"),
        format!("{model} · {provider}"),
        format!("session: {session_id}"),
        format!("Work at: {}", cwd.display()),
    ]
    .join("\n");

    let term_width = terminal_size().map(|(Width(w), _)| w as usize).unwrap_or(80);
    let width = term_width.clamp(36, 80).saturating_sub(12);

    println!("{greeting}");

    let mut stdout = BufWriter::new(io::stdout().lock());
    let _ = say(&info, width, &mut stdout);
    println!();
}
