use std::io;
use std::path::PathBuf;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Failed to read {file_path} file: {err}")]
    Io { file_path: PathBuf, err: io::Error },
    #[error("Failed to parse {file_path} frontmatter: {err}")]
    Yaml { file_path: PathBuf, err: serde_yaml::Error },
    #[error("Wrong makrdown format at {file_path} file")]
    WrongFormat { file_path: PathBuf },
}

impl Error {
    pub fn io(file_path: PathBuf, err: io::Error) -> Self {
        Self::Io { file_path, err }
    }

    pub fn yaml(file_path: PathBuf, err: serde_yaml::Error) -> Self {
        Self::Yaml { file_path, err }
    }

    pub fn wrong_format(file_path: PathBuf) -> Self {
        Self::WrongFormat { file_path }
    }
}
