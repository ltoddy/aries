use std::io::{BufWriter, stdout};

use aries_context::GlobalContext;
use aries_theme::Theme;

pub fn welcome(provider: &str, model: &str, context: &GlobalContext) {
    let theme = Theme::default();

    let name = env!("CARGO_PKG_NAME");
    let version = env!("CARGO_PKG_VERSION");

    let input = [
        format!("{} {}", name, version),
        format!("provider {provider}"),
        format!("model {model}"),
        format!("dir {}", context.current_dir.display()),
    ]
    .join("\n");

    let stdout = stdout();
    let width = 80;

    let writer = BufWriter::new(stdout.lock());

    if let Err(e) = ferris_says::say(&input, width, writer) {
        eprintln!("ferris_says error: {}", e);
        return;
    }

    println!();
    println!("{}", theme.dimmed("  /help for help  /exit to exit"));
}
