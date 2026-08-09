use aries_session::commands::BUILTIN_COMMANDS;
use dialoguer::Select;
use dialoguer::theme::ColorfulTheme;
use itertools::Itertools;
use rustyline::completion::{Completer, Pair};
use rustyline::hint::HistoryHinter;
use rustyline::{Context, Result};
use rustyline_derive::{Helper, Highlighter, Hinter, Validator};

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

pub fn show(prefix: &str) -> Option<String> {
    let filtered = BUILTIN_COMMANDS
        .iter()
        .filter(|(cmd, _, _)| format!("/{cmd}").starts_with(prefix))
        .map(|(cmd, desc, _)| (cmd, desc))
        .collect_vec();
    if filtered.is_empty() {
        return None;
    }

    let items = filtered.iter().map(|(cmd, desc)| format!("/{cmd} - {desc}")).collect_vec();
    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select a command")
        .default(0)
        .items(&items)
        .interact_opt()
        .ok()
        .flatten()?;

    Some(format!("/{}", filtered[selection].0))
}
