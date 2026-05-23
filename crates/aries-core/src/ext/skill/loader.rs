use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use futures::stream::{self, StreamExt};

use crate::ext::skill::SkillDefinition;
use crate::fs::walk_dirs;

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

        println!("roots: {:#?}", roots);

        Self { roots }
    }

    pub async fn load(self) -> anyhow::Result<Vec<SkillDefinition>> {
        let entries = walk_dirs(&self.roots, true, true)?;

        let file_paths = entries
            .into_iter()
            .filter(|entry| entry.file_name().eq(&Some(OsStr::new(Self::FILENAME))))
            .collect::<Vec<_>>();

        println!("file_paths is: {:#?}", file_paths);

        let skills = stream::iter(file_paths)
            .filter_map(|file_path| async move { SkillDefinition::parse(file_path).await.ok() })
            .collect()
            .await;

        Ok(skills)
    }
}
