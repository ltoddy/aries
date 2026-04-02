use rustyline::Context;
use rustyline::completion::{Completer, extract_word};
use rustyline::hint::Hinter;
use rustyline_derive::{Helper, Highlighter, Validator};

#[derive(Helper, Highlighter, Validator)]
pub struct CommandCompleter {
    commands: Vec<String>,
}

impl CommandCompleter {
    pub fn new() -> Self {
        Self {
            commands: vec![
                crate::commands::exit::NAME.to_string(),
                crate::commands::bash::NAME.to_string(),
                crate::commands::setup::NAME.to_string(),
                crate::commands::save_history::NAME.to_string(),
                "/clear".to_string(),
                "/help".to_string(),
            ],
        }
    }
}

impl Hinter for CommandCompleter {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> Option<String> {
        if line.is_empty() || pos < line.len() {
            return None;
        }

        if !line.starts_with('/') {
            return None;
        }

        self.commands.iter().find(|cmd| cmd.starts_with(line)).map(|cmd| cmd[line.len()..].to_string())
    }
}

impl Completer for CommandCompleter {
    type Candidate = String;

    fn complete(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> rustyline::Result<(usize, Vec<String>)> {
        let (start, word) = extract_word(line, pos, None, |c| c == ' ');

        let matches: Vec<String> = self.commands.iter().filter(|cmd| cmd.starts_with(word)).cloned().collect();

        Ok((start, matches))
    }
}
