use colored::Colorize;

use crate::context::GlobalContext;

pub fn welcome(model_name: &str, context: &GlobalContext) {
    let theme = context.theme;
    let pkg_name = {
        let name = env!("CARGO_PKG_NAME");
        let mut chars = name.chars();
        match chars.next() {
            None => String::new(),
            Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        }
    };

    let message = format!(
        "{} {}  model {}  dir {}",
        pkg_name,
        env!("CARGO_PKG_VERSION"),
        model_name,
        context.current_dir.display()
    );

    let mut output = Vec::new();
    let width = 80;
    if let Err(e) = ferris_says::say(&message, width, &mut output) {
        eprintln!("ferris_says error: {}", e);
        println!(
            "{} {} | model {} | dir {}",
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION"),
            model_name,
            context.current_dir.display()
        );
        return;
    }

    let output_str = String::from_utf8_lossy(&output);
    println!("{}", output_str.to_string().color(theme.black()));
    println!();
    let help_text = "  /help for help  /exit to exit".to_string();
    println!("{}", theme.dimmed(&help_text));
}
