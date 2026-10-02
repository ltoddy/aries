pub mod list;
pub mod prune;
pub mod resume;
pub mod run;

use std::time::Instant;

use clap::Subcommand;
use colored::Colorize;

use self::list::ListSessionsArgs;
use self::prune::PruneSessionsArgs;
use self::resume::ResumeSessionsArgs;

#[derive(Subcommand, Debug, Clone)]
pub enum SessionCommand {
    #[command(about = "List chat sessions")]
    List(ListSessionsArgs),
    #[command(about = "Delete old chat sessions")]
    Prune(PruneSessionsArgs),
    #[command(about = "Resume a previous chat session")]
    Resume(ResumeSessionsArgs),
}

const TERMINAL_WIDTH: usize = 80;

fn display_elapsed(start: Instant) {
    let elapsed = start.elapsed();

    let prefix = "─".repeat(5);
    let time = format!("⏱️  耗时: {:.2}s", elapsed.as_secs_f64());
    let remining_width = TERMINAL_WIDTH.saturating_sub(prefix.len() + time.len());
    let line = format!("{}{}{}", "─".repeat(5), time, "─".repeat(remining_width));
    println!("{}\n", line.dimmed());
}
