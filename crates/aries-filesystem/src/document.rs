use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde::de::DeserializeOwned;
use thiserror::Error;

const DELIMITER: &str = "---";

#[derive(Debug, Clone)]
pub struct FrontmatterDocument<F>
where
    F: Serialize + DeserializeOwned,
{
    pub location: PathBuf,
    pub frontmatter: F,
    pub body: String,
}

impl<F> FrontmatterDocument<F>
where
    F: Serialize + DeserializeOwned,
{
    pub fn new(location: impl AsRef<Path>, frontmatter: F, body: impl Into<String>) -> Self {
        let location = location.as_ref().to_owned();
        let body = body.into();

        Self { location, frontmatter, body }
    }

    pub async fn read(file_path: impl AsRef<Path>) -> Result<Self, DocumentError> {
        let file_path = file_path.as_ref();
        let location = file_path.to_owned();
        let content = tokio::fs::read_to_string(file_path)
            .await
            .map_err(|err| DocumentError::io(&location, err))?;

        let mut parts = content.splitn(3, DELIMITER);
        parts.next();

        match (parts.next(), parts.next()) {
            (Some(frontmatter), Some(body)) => {
                let frontmatter = serde_yaml::from_str::<F>(frontmatter)
                    .map_err(|err| DocumentError::yaml(&location, err))?;

                Ok(Self::new(location, frontmatter, body))
            },
            _ => Err(DocumentError::wrong_format(&location)),
        }
    }

    pub async fn write(&self) -> Result<(), DocumentError> {
        let frontmatter =
            serde_yaml::to_string(&self.frontmatter).map_err(DocumentError::yaml_serialize)?;

        let content = [DELIMITER, &frontmatter, DELIMITER, &self.body].join("\n");
        tokio::fs::write(&self.location, content)
            .await
            .map_err(|err| DocumentError::io(&self.location, err))?;
        Ok(())
    }
}

#[derive(Error, Debug)]
pub enum DocumentError {
    #[error("{}: {err}", .file_path.display())]
    Io {
        file_path: PathBuf,
        #[source]
        err: io::Error,
    },
    #[error("{}: invalid YAML frontmatter — {err}", .file_path.display())]
    Yaml {
        file_path: PathBuf,
        #[source]
        err: serde_yaml::Error,
    },
    #[error("failed to serialize frontmatter to YAML: {0}")]
    YamlSerialize(#[source] serde_yaml::Error),
    #[error("{}: missing YAML frontmatter — expected content wrapped in '---' delimiters", .file_path.display())]
    WrongFormat { file_path: PathBuf },
}

impl DocumentError {
    pub fn io(file_path: impl AsRef<Path>, err: io::Error) -> Self {
        let file_path = file_path.as_ref().to_owned();
        Self::Io { file_path, err }
    }

    pub fn yaml(file_path: impl AsRef<Path>, err: serde_yaml::Error) -> Self {
        let file_path = file_path.as_ref().to_owned();
        Self::Yaml { file_path, err }
    }

    pub fn yaml_serialize(err: serde_yaml::Error) -> Self {
        Self::YamlSerialize(err)
    }

    pub fn wrong_format(file_path: impl AsRef<Path>) -> Self {
        let file_path = file_path.as_ref().to_owned();
        Self::WrongFormat { file_path }
    }
}
