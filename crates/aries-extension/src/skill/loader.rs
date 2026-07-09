use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use aries_filesystem::document::FrontmatterDocument;
use aries_filesystem::walk::walk_dirs;
use futures::stream::{self, StreamExt};

use crate::skill::SkillDefinition;

pub struct SkillsLoader {
    roots: Vec<PathBuf>,
}

impl SkillsLoader {
    pub const FILENAME: &str = "SKILL.md";

    pub fn new(cwd: impl AsRef<Path>) -> Self {
        let cwd = cwd.as_ref();
        let home_dir = std::env::home_dir().unwrap_or_else(|| PathBuf::from("~"));

        let roots =
            vec![cwd.join(".agents").join("skills"), home_dir.join(".agents").join("skills")];

        Self { roots }
    }

    pub async fn load(self) -> anyhow::Result<Vec<SkillDefinition>> {
        let entries = walk_dirs(&self.roots, true, true)?;

        let file_paths = entries
            .into_iter()
            .filter(|entry| entry.file_name().eq(&Some(OsStr::new(Self::FILENAME))))
            .collect::<Vec<_>>();

        let skills = stream::iter(file_paths)
            .filter_map(|file_path| async move { FrontmatterDocument::read(file_path).await.ok() })
            .map(|doc| SkillDefinition::new(doc.location, doc.frontmatter, doc.body))
            .collect()
            .await;

        Ok(skills)
    }
}
