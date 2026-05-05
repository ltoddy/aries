use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug)]
pub struct ManifestFile {
    file_path: PathBuf,
    manifest: Manifest,
}

impl ManifestFile {
    const FILENAME: &str = "sessions-manifest.yaml";

    pub async fn new(dir: impl AsRef<Path>) -> Self {
        let dir = dir.as_ref();
        let file_path = dir.join(Self::FILENAME);

        let manifest = Self::load_manifest(&file_path).await.unwrap_or_default();

        Self { file_path, manifest }
    }

    pub async fn append(
        &mut self,
        session_id: String,
        title: String,
        project_dir: PathBuf,
    ) -> anyhow::Result<(), ManifestError> {
        let entry = Entry { session_id, title, project_dir };
        self.manifest.push(entry);
        self.save().await
    }

    pub async fn update_title(
        &mut self,
        session_id: String,
        title: String,
    ) -> anyhow::Result<bool, ManifestError> {
        if let Some(entry) = self.manifest.iter_mut().find(|entry| entry.session_id == session_id) {
            entry.title = title;
            self.save().await?;
            return Ok(true);
        }
        Ok(false)
    }

    pub async fn delete(&mut self, session_ids: &[String]) -> anyhow::Result<(), ManifestError> {
        self.manifest.retain(|entry| !session_ids.contains(&entry.session_id));
        self.save().await
    }

    pub fn list(&self, project_dir: &Path) -> Vec<&Entry> {
        self.manifest.iter().filter(|entry| entry.project_dir == project_dir).collect()
    }

    pub fn all(&self) -> &[Entry] {
        &self.manifest
    }

    pub fn file_path(&self) -> &Path {
        &self.file_path
    }

    async fn load_manifest(file_path: impl AsRef<Path>) -> anyhow::Result<Manifest, ManifestError> {
        let file_path = file_path.as_ref();

        let manifest = tokio::fs::read_to_string(file_path)
            .await
            .map(|content| serde_yaml::from_str::<Manifest>(&content))??;

        Ok(manifest)
    }

    async fn save(&self) -> anyhow::Result<(), ManifestError> {
        let content = serde_yaml::to_string(&self.manifest)?;
        tokio::fs::write(&self.file_path, &content).await?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub session_id: String,
    pub title: String,
    pub project_dir: PathBuf,
}

#[derive(Debug, Error)]
pub enum ManifestError {
    #[error("failed to read/write manifest file: {0}")]
    IO(#[from] io::Error),
    #[error("failed to parse manifest yaml: {0}")]
    Yaml(#[from] serde_yaml::Error),
}

pub type Manifest = Vec<Entry>;
