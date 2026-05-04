use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};

use rustyline::history::DefaultHistory;
use rustyline::{Config, Editor};

use crate::commands::completer::CommandCompleter;

pub struct InputReader {
    editor: Editor<CommandCompleter, DefaultHistory>,
    file_path: PathBuf,
}

impl InputReader {
    pub fn new(dir: impl AsRef<Path>) -> anyhow::Result<Self> {
        let file_path = dir.as_ref().join("history.txt");

        let config = Config::builder().auto_add_history(true).build();
        let mut rl = Editor::with_config(config)?;
        rl.set_helper(Some(CommandCompleter::new()));
        let _ = rl.load_history(&file_path);

        Ok(Self { editor: rl, file_path })
    }
}

impl Drop for InputReader {
    fn drop(&mut self) {
        let _ = self.editor.save_history(&self.file_path);
    }
}

impl Deref for InputReader {
    type Target = Editor<CommandCompleter, DefaultHistory>;

    fn deref(&self) -> &Self::Target {
        &self.editor
    }
}

impl DerefMut for InputReader {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.editor
    }
}
