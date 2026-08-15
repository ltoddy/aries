#[cfg(test)]
mod tests;

use std::fmt::{Display, Formatter};
use std::io;
use std::path::{Path, PathBuf};

use aries_filesystem::document::FrontmatterDocument;
use itertools::Itertools;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct MemoryStore {
    dir: PathBuf,
}

impl MemoryStore {
    const MANIFEST_FILENAME: &str = "MEMORY.md";

    pub async fn new(dir: impl AsRef<Path>) -> Self {
        let dir = dir.as_ref();
        _ = tokio::fs::create_dir_all(dir).await;

        Self { dir: dir.to_path_buf() }
    }

    pub async fn read_memory(&self, file_name: impl Into<String>) -> Option<String> {
        let file_name = file_name.into();
        let file_path = self.dir.join(file_name);

        FrontmatterDocument::<MemoryFrontmatter>::read(&file_path)
            .await
            .ok()
            .map(|document| document.body.trim().to_owned())
    }

    pub async fn read_manifest(&self) -> io::Result<Option<String>> {
        let file_path = self.dir.join(Self::MANIFEST_FILENAME);

        match tokio::fs::read_to_string(&file_path).await.map(|content| content.trim().to_owned()) {
            Ok(content) if content.is_empty() => Ok(None),
            Ok(content) => Ok(Some(content.lines().filter(|l| !l.trim().is_empty()).join("\n"))),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err),
        }
    }

    pub async fn scan(&self) -> Vec<Memory> {
        let Ok(mut entries) = tokio::fs::read_dir(&self.dir).await else { return vec![] };

        let mut memories = vec![];
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();

            let Some(file_name) = path.file_name() else {
                continue;
            };
            let file_name = file_name.to_string_lossy().into_owned();
            if file_name == Self::MANIFEST_FILENAME {
                continue;
            }
            if !file_name.ends_with(".md") {
                continue;
            }

            if let Ok(doc) = FrontmatterDocument::<MemoryFrontmatter>::read(&path).await {
                memories.push(Memory::new(path, file_name, doc.frontmatter, doc.body))
            }
        }

        memories
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

#[derive(Debug, Clone)]
pub struct Memory {
    pub location: PathBuf,
    pub file_name: String,
    pub frontmatter: MemoryFrontmatter,
    pub body: String,
}

impl Memory {
    pub fn new(
        location: impl AsRef<Path>,
        file_name: impl Into<String>,
        frontmatter: MemoryFrontmatter,
        body: impl Into<String>,
    ) -> Self {
        let location = location.as_ref();
        let file_name = file_name.into();
        let body = body.into();

        Self { location: location.to_path_buf(), file_name, frontmatter, body }
    }

    pub fn to_retriever_line(&self) -> String {
        format!(
            "- [{}] ({}) — {}",
            self.file_name, self.frontmatter.memory_type, self.frontmatter.description
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryFrontmatter {
    pub name: String,
    pub description: String,
    #[serde(rename = "type")]
    pub memory_type: MemoryType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum MemoryType {
    User,
    Feedback,
    Project,
    Reference,
}

impl Display for MemoryType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            MemoryType::User => "user".fmt(f),
            MemoryType::Feedback => "feedback".fmt(f),
            MemoryType::Project => "project".fmt(f),
            MemoryType::Reference => "reference".fmt(f),
        }
    }
}
