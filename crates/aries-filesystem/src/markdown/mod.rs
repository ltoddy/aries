mod error;

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde::de::DeserializeOwned;

pub use self::error::Error;

pub struct MarkdownFile {
    file_path: PathBuf,
}

impl MarkdownFile {
    const DELIMITER: &str = "---";

    pub fn new(file_path: impl AsRef<Path>) -> Self {
        let file_path = file_path.as_ref().to_path_buf();

        Self { file_path }
    }

    pub async fn read<F>(&self) -> Result<Markdown<F>, Error>
    where
        F: Serialize + DeserializeOwned,
    {
        let content = tokio::fs::read_to_string(&self.file_path)
            .await
            .map_err(|err| Error::io(self.file_path.clone(), err))?;

        let mut parts = content.splitn(3, Self::DELIMITER);
        parts.next();

        match (parts.next(), parts.next()) {
            (Some(frontmatter), Some(body)) => {
                let frontmatter = serde_yaml::from_str::<F>(frontmatter)
                    .map_err(|err| Error::yaml(self.file_path.clone(), err))?;

                Ok(Markdown::new(&self.file_path, frontmatter, body))
            },
            _ => Err(Error::wrong_format(self.file_path.clone())),
        }
    }

    pub async fn write<F>(&self, markdown: Markdown<F>) -> Result<(), Error>
    where
        F: Serialize + DeserializeOwned,
    {
        let frontmatter = serde_yaml::to_string(&markdown.frontmatter)
            .map_err(|err| Error::yaml(self.file_path.clone(), err))?;

        let content = [frontmatter, String::from("\n"), markdown.body].join("\n");

        tokio::fs::write(&self.file_path, content)
            .await
            .map_err(|err| Error::io(self.file_path.clone(), err))?;

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Markdown<F>
where
    F: Serialize + DeserializeOwned,
{
    pub location: PathBuf,
    pub frontmatter: F,
    pub body: String,
}

impl<F> Markdown<F>
where
    F: Serialize + DeserializeOwned,
{
    pub fn new(location: impl AsRef<Path>, frontmatter: F, body: impl Into<String>) -> Self {
        let location = location.as_ref().to_path_buf();
        let body = body.into();

        Self { location, frontmatter, body }
    }
}
