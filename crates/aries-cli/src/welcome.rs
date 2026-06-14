use aries_context::GlobalContext;
use terminal_size::{Width, terminal_size};

use crate::theme::Theme;

pub fn welcome(
    provider: impl Into<String>,
    model: impl Into<String>,
    session_id: impl Into<String>,
    context: &GlobalContext,
) {
    let theme = Theme::default();

    let provider = provider.into();
    let model = model.into();
    let session_id = session_id.into();

    let name = env!("CARGO_BIN_NAME");
    let version = env!("CARGO_PKG_VERSION");

    let title = format!(" {name} v{version} ");
    let greeting = if context.user.is_empty() {
        "Welcome!".to_string()
    } else {
        format!("Welcome, {}!", context.user)
    };
    let mascot = ["▄▀▀▙▟▀▀▄", " ▝▜██▛▘", "   ▘▘"];
    let info = format!("{model} · {provider}");
    let sid = format!("session: {session_id}");
    let dir = context.current_dir.display().to_string();

    let term_width = terminal_size().map(|(Width(w), _)| w as usize).unwrap_or(80);
    let inner = term_width.clamp(36, 50);

    let blank = format!("{}{}{}", theme.dimmed("│"), " ".repeat(inner), theme.dimmed("│"));

    // ╭─── title ──────────────────╮
    let title_len = title.chars().count();
    let remaining = inner.saturating_sub(title_len);
    let left = 3;
    let right = remaining.saturating_sub(left);
    print!("{}", theme.dimmed(&format!("╭{}", "─".repeat(left))));
    print!("{}", theme.cyan_text(&title));
    println!("{}", theme.dimmed(&format!("{}╮", "─".repeat(right))));

    println!("{blank}");
    print_centered(&theme, &greeting, inner, |s| s.to_string());
    println!("{blank}");

    for line in &mascot {
        print_centered(&theme, line, inner, |s| format!("{}", theme.cyan_text(s)));
    }

    println!("{blank}");
    print_centered(&theme, &info, inner, |s| s.to_string());
    print_centered(&theme, &sid, inner, |s| format!("{}", theme.dimmed(s)));
    print_centered(&theme, &dir, inner, |s| format!("{}", theme.dimmed(s)));

    println!("{}", theme.dimmed(&format!("╰{}╯", "─".repeat(inner))));
}

fn print_centered(theme: &Theme, text: &str, width: usize, style: impl Fn(&str) -> String) {
    let len = text.chars().count();
    let lp = width.saturating_sub(len) / 2;
    let rp = width.saturating_sub(len).saturating_sub(lp);
    println!(
        "{}{}{}{}{}",
        theme.dimmed("│"),
        " ".repeat(lp),
        style(text),
        " ".repeat(rp),
        theme.dimmed("│"),
    );
}
