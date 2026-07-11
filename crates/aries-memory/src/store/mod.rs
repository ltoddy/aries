use std::fmt::{Display, Formatter};
use std::io;
use std::path::{Path, PathBuf};

use aries_filesystem::document::FrontmatterDocument;
use itertools::Itertools;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use tracing::{info, warn};

#[derive(Clone)]
pub struct MemoryStore {
    dir: PathBuf,
}

impl MemoryStore {
    const MANIFEST_FILENAME: &str = "MEMORY.md";

    pub async fn new(root_dir: impl AsRef<Path>, project_path: impl AsRef<Path>) -> Self {
        let root_dir = root_dir.as_ref();
        let dir = root_dir.join("projects").join(sanitize_project_path(project_path));

        if let Err(err) = tokio::fs::create_dir_all(&dir).await {
            warn!(error = %err, path = %dir.display(), "failed to create memory directory");
        }

        Self { dir }
    }

    pub async fn write_memory(
        &self,
        frontmatter: MemoryFrontmatter,
        body: impl Into<String>,
    ) -> Result<(), aries_filesystem::document::DocumentError> {
        let filename = to_filename(&frontmatter.name);
        let file_path = self.dir.join(&filename);

        let document = FrontmatterDocument::new(&file_path, frontmatter, body);
        info!(file_path = %file_path.display(), "writing memory");
        document.write().await
    }

    pub async fn read_manifest(&self) -> io::Result<Option<String>> {
        let file_path = self.dir.join(Self::MANIFEST_FILENAME);

        match tokio::fs::read_to_string(&file_path).await.map(|content| content.trim().to_owned()) {
            Ok(content) if content.is_empty() => Ok(None),
            Ok(content) => Ok(Some(content.lines().filter(|l| l.trim().is_empty()).join("\n"))),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err),
        }
    }

    pub async fn append_to_manifest(&self, entry: ManifestEntry) -> io::Result<()> {
        let file_path = self.dir.join(Self::MANIFEST_FILENAME);

        let line = format!("{}\n", entry.format());

        let mut file = OpenOptions::new().append(true).create(true).open(&file_path).await?;
        file.write_all(line.as_bytes()).await?;

        info!(file_path = %file_path.display(), entry = %entry.filename, "appending to manifest");
        Ok(())
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

#[derive(Debug, Clone)]
pub struct ManifestEntry {
    pub filename: String,
    pub description: String,
}

impl ManifestEntry {
    pub fn new(filename: impl Into<String>, description: impl Into<String>) -> Self {
        let filename = filename.into();
        let description = description.into();

        Self { filename, description }
    }

    pub fn format(&self) -> String {
        format!("- [{}]({}) — {}\n", self.filename, self.filename, self.description)
    }
}

fn to_filename(name: &str) -> String {
    let slug = name
        .chars()
        .map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '_' })
        .collect::<String>();

    let trimmed = slug.trim_matches('_').replace("__", "_");
    format!("{}.md", trimmed)
}

fn sanitize_project_path(path: impl AsRef<Path>) -> String {
    let path = path.as_ref();
    let path = path.to_string_lossy();

    path.replace(std::path::MAIN_SEPARATOR, "_")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryFrontmatter {
    pub name: String,
    pub description: String,
    #[serde(rename = "type")]
    pub memory_type: MemoryType,
}

impl MemoryFrontmatter {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        memory_type: MemoryType,
    ) -> Self {
        let name = name.into();
        let description = description.into();

        Self { name, description, memory_type }
    }
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
