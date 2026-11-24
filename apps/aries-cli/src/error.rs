use std::path::PathBuf;

use aries_init::SettingError;
use aries_runtime::agent::AriesError;
use aries_runtime::registry::RegistryError;
use rustyline::error::ReadlineError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Setting(SettingError),

    #[error(transparent)]
    Registry(RegistryError),

    #[error(transparent)]
    Prompt(AriesError),

    #[error(transparent)]
    Readline(ReadlineError),

    #[error(transparent)]
    Dialog(dialoguer::Error),

    #[error(transparent)]
    Acp(agent_client_protocol::Error),

    #[error(transparent)]
    Storage(toasty::Error),

    #[error("failed to load setting; run `aries setup` to initialize the configuration")]
    LoadSetting(#[source] SettingError),

    #[error("{operation} {}", root_dir.display())]
    Database { operation: &'static str, root_dir: PathBuf, source: toasty::Error },

    #[error("failed to delete sessions from local storage: {session_ids:?}")]
    DeleteSessions { session_ids: Vec<String>, source: toasty::Error },

    #[error("setting tree-sitter bash language")]
    BashLanguage(#[source] tree_sitter::LanguageError),
}

impl Error {
    pub fn setting(source: SettingError) -> Self {
        Self::Setting(source)
    }

    pub fn registry(source: RegistryError) -> Self {
        Self::Registry(source)
    }

    pub fn prompt(source: AriesError) -> Self {
        Self::Prompt(source)
    }

    pub fn readline(source: ReadlineError) -> Self {
        Self::Readline(source)
    }

    pub fn dialog(source: dialoguer::Error) -> Self {
        Self::Dialog(source)
    }

    pub fn acp(source: agent_client_protocol::Error) -> Self {
        Self::Acp(source)
    }

    pub fn storage(source: toasty::Error) -> Self {
        Self::Storage(source)
    }

    pub fn load_setting(source: SettingError) -> Self {
        Self::LoadSetting(source)
    }

    pub fn database(
        operation: &'static str,
        root_dir: impl Into<PathBuf>,
        source: toasty::Error,
    ) -> Self {
        Self::Database { operation, root_dir: root_dir.into(), source }
    }

    pub fn delete_sessions(session_ids: Vec<String>, source: toasty::Error) -> Self {
        Self::DeleteSessions { session_ids, source }
    }

    pub fn bash_language(source: tree_sitter::LanguageError) -> Self {
        Self::BashLanguage(source)
    }
}
