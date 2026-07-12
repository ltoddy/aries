mod instruction;
pub mod memory;
mod repo;
mod skill;

use std::path::Path;

use aries_extension::skill::definition::SkillDefinition;

pub async fn sections(
    cwd: impl AsRef<Path>,
    model: impl Into<String>,
    available_skills: &[SkillDefinition],
    memory: Option<&str>,
) -> Vec<String> {
    let cwd = cwd.as_ref();
    let mut sections = Vec::new();

    sections.push(aries_preamble::env::section(cwd, model));

    if let Some(section) = repo::render(cwd).await {
        sections.push(section);
    }

    if !available_skills.is_empty() {
        sections.push(skill::render(available_skills));
    }

    if let Some(mem) = memory {
        sections.push(mem.to_owned());
    }

    let loader = instruction::AgentsmdFileLoader::new(cwd);
    if let Some(content) = loader.read().await {
        sections.push(format!("Instructions from: {}\n{content}", loader.file_path().display()));
    }

    sections
}
