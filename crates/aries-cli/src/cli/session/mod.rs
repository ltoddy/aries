pub mod list;
pub mod prune;
pub mod resume;
pub mod run;

use std::time::Instant;

use clap::Subcommand;
use terminal_size::{Width, terminal_size};

use self::list::ListSessionsArgs;
use self::prune::PruneSessionsArgs;
use self::resume::ResumeSessionsArgs;
use crate::theme::Theme;

#[derive(Subcommand, Debug, Clone)]
pub enum SessionCommand {
    #[command(about = "List chat sessions")]
    List(ListSessionsArgs),
    #[command(about = "Delete old chat sessions")]
    Prune(PruneSessionsArgs),
    #[command(about = "Resume a previous chat session")]
    Resume(ResumeSessionsArgs),
}

fn display_elapsed(start: Instant, theme: &Theme) {
    let elapsed = start.elapsed();
    let terminal_width = terminal_size().map(|(Width(w), _)| w as usize).unwrap_or(80);

    let prefix = "─".repeat(5);
    let time = format!("⏱️  耗时: {:.2}s", elapsed.as_secs_f64());
    let remining_width = terminal_width.saturating_sub(prefix.len() + time.len());
    let line = format!("{}{}{}", "─".repeat(5), time, "─".repeat(remining_width));
    println!("{}\n", theme.dimmed(&line));
}
