use dialoguer::Select;
use dialoguer::theme::ColorfulTheme;
use rustyline::completion::{Completer, Pair};
use rustyline::hint::HistoryHinter;
use rustyline::{Context, Result};
use rustyline_derive::{Helper, Highlighter, Hinter, Validator};

use crate::commands::{bash, clear_history, exit, save_history, setup};

#[derive(Helper, Highlighter, Hinter, Validator)]
pub struct CommandCompleter {
    #[rustyline(Hinter)]
    hinter: HistoryHinter,
}

impl CommandCompleter {
    pub fn new() -> Self {
        Self { hinter: HistoryHinter {} }
    }
}

impl Completer for CommandCompleter {
    type Candidate = Pair;

    fn complete(&self, line: &str, _pos: usize, _ctx: &Context<'_>) -> Result<(usize, Vec<Pair>)> {
        if line.starts_with('/')
            && let Some(selected) = show(line)
        {
            return Ok((0, vec![Pair { display: selected.clone(), replacement: selected }]));
        }

        Ok((0, vec![]))
    }
}

const COMMANDS: &[(&str, &str)] = &[
    (exit::NAME, "Exit Aries"),
    (bash::NAME, "Run a bash command"),
    (setup::NAME, "Open configuration setup"),
    (save_history::NAME, "Save chat history to file"),
    (clear_history::NAME, "Clear chat history"),
];

pub fn show(prefix: &str) -> Option<String> {
    let filtered: Vec<(&str, &str)> =
        COMMANDS.iter().filter(|(cmd, _)| cmd.starts_with(prefix)).copied().collect();

    if filtered.is_empty() {
        return None;
    }

    let items: Vec<String> = filtered.iter().map(|(cmd, desc)| format!("{cmd}  {desc}")).collect();

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select a command")
        .default(0)
        .items(&items)
        .interact_opt()
        .ok()
        .flatten()?;

    Some(filtered[selection].0.to_string())
}
