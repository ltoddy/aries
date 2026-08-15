use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use aries_filesystem::document::FrontmatterDocument;
use aries_filesystem::walk::walk_dirs;
use futures::stream::{self, StreamExt};
use itertools::Itertools;

use crate::skill::SkillDefinition;

#[derive(Debug)]
pub struct SkillsLoader {
    roots: Vec<PathBuf>,
}

impl SkillsLoader {
    pub const FILENAME: &str = "SKILL.md";

    pub fn new(cwd: impl AsRef<Path>, home_dir: impl AsRef<Path>) -> Self {
        let cwd = cwd.as_ref();
        let home_dir = home_dir.as_ref();

        let roots = vec![
            home_dir.join(".agents").join("skills"),
            home_dir.join(".agents").join("plugins").join("skills"),
            cwd.join(".agents").join("skills"),
        ]
        .into_iter()
        .unique()
        .collect_vec();

        Self { roots }
    }

    pub async fn load(self) -> Vec<SkillDefinition> {
        let Ok(entries) = walk_dirs(&self.roots, true, true) else { return vec![] };

        let file_paths = entries
            .into_iter()
            .filter(|entry| entry.file_name().eq(&Some(OsStr::new(Self::FILENAME))))
            .collect::<Vec<_>>();

        let skills = stream::iter(file_paths)
            .filter_map(|file_path| async move { FrontmatterDocument::read(file_path).await.ok() })
            .map(|doc| SkillDefinition::new(doc.location, doc.frontmatter, doc.body))
            .collect::<Vec<_>>()
            .await;

        let mut skills = skills
            .into_iter()
            .sorted_by(|a, b| a.frontmatter.name.cmp(&b.frontmatter.name))
            .collect::<Vec<_>>();
        skills.dedup_by(|a, b| a.frontmatter.name == b.frontmatter.name);
        skills
    }
}
